use crate::error::{VoicyError, VoicyResult};
use enigo::{Enigo, Keyboard, Settings};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// Optimized typing system with single worker thread
pub struct TypingQueue {
    sender: Option<Sender<TypingCommand>>,
    worker_handle: Option<thread::JoinHandle<()>>,
    use_direct_execution: bool,
}

#[derive(Debug)]
enum TypingCommand {
    Type { text: String, add_space: bool },
    Shutdown,
}

impl TypingQueue {
    pub fn new(use_direct_execution: bool) -> Self {
        if use_direct_execution {
            // Direct execution mode: use a single worker thread instead of spawning per-operation
            let (sender, receiver) = mpsc::channel();
            
            let worker_handle = thread::spawn(move || {
                Self::worker_loop(receiver);
            });
            
            Self {
                sender: Some(sender),
                worker_handle: Some(worker_handle),
                use_direct_execution,
            }
        } else {
            // Main thread mode: no worker needed
            Self {
                sender: None,
                worker_handle: None,
                use_direct_execution,
            }
        }
    }
    
    fn worker_loop(receiver: Receiver<TypingCommand>) {
        // Create Enigo once for the entire worker lifetime
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => {
                println!("✅ Typing worker initialized");
                e
            }
            Err(e) => {
                eprintln!("❌ Failed to initialize typing worker: {}", e);
                return;
            }
        };
        
        // Track consecutive failures for circuit breaker pattern
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 5;
        
        // Process commands from the queue
        while let Ok(command) = receiver.recv() {
            match command {
                TypingCommand::Type { text, add_space } => {
                    // Circuit breaker: skip if too many failures
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        // Try to recreate Enigo after cooldown
                        thread::sleep(Duration::from_millis(500));
                        match Enigo::new(&Settings::default()) {
                            Ok(new_enigo) => {
                                enigo = new_enigo;
                                consecutive_failures = 0;
                                println!("🔄 Typing system recovered");
                            }
                            Err(e) => {
                                eprintln!("❌ Still cannot initialize typing: {}", e);
                                continue;
                            }
                        }
                    }
                    
                    // Perform typing with simple retry
                    let success = Self::type_with_retry(&mut enigo, &text, add_space);
                    
                    if success {
                        consecutive_failures = 0;
                    } else {
                        consecutive_failures += 1;
                        if consecutive_failures == MAX_CONSECUTIVE_FAILURES {
                            eprintln!("⚠️ Typing system entering recovery mode");
                        }
                    }
                }
                TypingCommand::Shutdown => {
                    println!("🛑 Typing worker shutting down");
                    break;
                }
            }
        }
    }
    
    fn type_with_retry(enigo: &mut Enigo, text: &str, add_space: bool) -> bool {
        const MAX_RETRIES: u32 = 2;
        
        for attempt in 0..=MAX_RETRIES {
            let mut success = true;
            
            // Add space if requested
            if add_space {
                if let Err(e) = enigo.text(" ") {
                    if attempt == MAX_RETRIES {
                        eprintln!("❌ Failed to type space: {}", e);
                    }
                    success = false;
                }
            }
            
            // Type the main text
            if !text.is_empty() && success {
                match enigo.text(text) {
                    Ok(()) => return true, // Success!
                    Err(e) => {
                        if attempt == MAX_RETRIES {
                            eprintln!("❌ Failed to type text: {}", e);
                        }
                        success = false;
                    }
                }
            }
            
            if success {
                return true;
            }
            
            // Exponential backoff before retry: 10ms, 20ms, 40ms
            if attempt < MAX_RETRIES {
                thread::sleep(Duration::from_millis(10 << attempt));
            }
        }
        
        false
    }
    
    pub fn queue_typing(&self, text: String, add_space: bool) -> VoicyResult<()> {
        // Skip empty operations
        if text.is_empty() && !add_space {
            return Ok(());
        }
        
        if let Some(ref sender) = self.sender {
            // Capture length for logging before moving text
            let text_len = text.len();
            sender
                .send(TypingCommand::Type { text, add_space })
                .map_err(|e| VoicyError::WindowOperationFailed(
                    format!("Typing worker disconnected: {}", e)
                ))?;

            if text_len > 0 {
                println!("💬 Queued typing ({} chars)", text_len);
            }
        } else {
            // Main thread mode - execute directly with cached Enigo
            self.execute_on_main_thread(text, add_space)?;
        }
        
        Ok(())
    }
    
    fn execute_on_main_thread(&self, text: String, add_space: bool) -> VoicyResult<()> {
        // Create Enigo instance for this operation (can't cache on macOS due to Send constraints)
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| VoicyError::WindowOperationFailed(
                format!("Failed to create Enigo: {}", e)
            ))?;
        
        // Type with error handling
        if add_space {
            enigo.text(" ").map_err(|e| 
                VoicyError::WindowOperationFailed(format!("Failed to type space: {}", e))
            )?;
        }
        
        if !text.is_empty() {
            enigo.text(&text).map_err(|e|
                VoicyError::WindowOperationFailed(format!("Failed to type text: {}", e))
            )?;
            println!("💬 Typed: \"{}\"", text);
        }
        
        Ok(())
    }
    
    pub fn process_queue(&self) -> VoicyResult<usize> {
        // Only relevant for main thread mode without worker
        if self.sender.is_some() {
            return Ok(0); // Worker handles everything
        }
        
        // In main thread mode, typing is synchronous, so nothing to process
        Ok(0)
    }
    
    pub fn initialize_on_main_thread(&self) -> VoicyResult<()> {
        if self.sender.is_some() {
            println!("✅ Typing queue using optimized worker thread");
            return Ok(());
        }
        
        // Test that we can create Enigo on main thread
        let _test = Enigo::new(&Settings::default())
            .map_err(|e| VoicyError::WindowOperationFailed(
                format!("Failed to initialize Enigo: {}", e)
            ))?;
        
        println!("✅ Typing queue initialized on main thread");
        Ok(())
    }
}

impl Drop for TypingQueue {
    fn drop(&mut self) {
        // Clean shutdown of worker thread
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(TypingCommand::Shutdown);
        }
        
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Clone for TypingQueue {
    fn clone(&self) -> Self {
        // For cloning, we share the same worker thread
        Self {
            sender: self.sender.clone(),
            worker_handle: None, // Clones don't own the worker
            use_direct_execution: self.use_direct_execution,
        }
    }
}

// Keep diagnostic function for compatibility
pub fn run_typing_diagnostic() {
    println!("🔍 Running typing diagnostic...");
    
    println!("\n1. Testing Enigo initialization...");
    match Enigo::new(&Settings::default()) {
        Ok(mut enigo) => {
            println!("   ✅ Enigo initialized successfully");
            
            println!("\n2. Testing basic typing (5-second delay)...");
            println!("   📋 Please switch to a text editor (TextEdit, Notes, etc.)");
            println!("   ⏰ Typing test will start in 5 seconds...");
            
            for i in (1..=5).rev() {
                println!("   ⏳ {}...", i);
                thread::sleep(Duration::from_secs(1));
            }
            
            println!("   🚀 Attempting to type...");
            
            match enigo.text("Hello from Voicy diagnostic test!") {
                Ok(()) => {
                    println!("   ✅ Enigo.text() returned successfully");
                    println!("   ❓ If you don't see text in your editor, it's a permissions issue");
                }
                Err(e) => {
                    println!("   ❌ Enigo.text() failed with error: {}", e);
                }
            }
            
            println!("\n3. Testing individual key simulation...");
            thread::sleep(Duration::from_millis(500));
            
            let test_chars = ['T', 'e', 's', 't'];
            for ch in test_chars {
                match enigo.key(enigo::Key::Unicode(ch), enigo::Direction::Click) {
                    Ok(()) => println!("   ✅ Key '{}' sent successfully", ch),
                    Err(e) => println!("   ❌ Key '{}' failed: {}", ch, e),
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
        Err(e) => {
            println!("   ❌ Failed to initialize Enigo: {}", e);
        }
    }
    
    println!("\n4. System Information:");
    println!("   📱 Platform: macOS");
    println!("   🔒 Accessibility permissions required");
    println!("\n✅ Diagnostic complete!");
}
