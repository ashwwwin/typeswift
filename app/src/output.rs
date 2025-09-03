use crate::error::{VoicyError, VoicyResult};
use enigo::{Enigo, Keyboard, Settings};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};
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
    Type { op_id: u64, text: String, add_space: bool },
    Shutdown,
}

impl TypingQueue {
    pub fn new(use_direct_execution: bool) -> Self {
        println!("🧵 TypingQueue init: worker_thread={}", use_direct_execution);
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
        println!("🧵 Typing worker started");
        // Track consecutive failures for diagnostics
        let mut consecutive_failures = 0u32;
        const MAX_CONSECUTIVE_FAILURES: u32 = 5;

        while let Ok(command) = receiver.recv() {
            match command {
                TypingCommand::Type { op_id, text, add_space } => {
                    println!(
                        "✉️  Typing worker received op_id={}, len={}, add_space={}",
                        op_id,
                        text.len(),
                        add_space
                    );
                    // Create a fresh Enigo instance per operation to avoid stale event sources
                    let mut enigo = match Enigo::new(&Settings::default()) {
                        Ok(e) => {
                            println!("🔧 Enigo created for op_id={}", op_id);
                            e
                        }
                        Err(e) => {
                            eprintln!("❌ Failed to initialize Enigo (op_id={}): {}", op_id, e);
                            consecutive_failures = consecutive_failures.saturating_add(1);
                            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                                eprintln!("⚠️ Repeated typing failures ({}).", consecutive_failures);
                            }
                            continue;
                        }
                    };

                    let success = Self::type_with_retry(&mut enigo, &text, add_space);
                    println!("🏷️  op_id={} typing result: {}", op_id, success);
                    if success {
                        println!("🎉 op_id={} typing complete", op_id);
                    }
                    if success {
                        consecutive_failures = 0;
                    } else {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                            eprintln!("⚠️ Repeated typing failures ({}).", consecutive_failures);
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
            println!("⌨️  Attempt {}/{} (len={}, add_space={})", attempt + 1, MAX_RETRIES + 1, text.len(), add_space);
            // Add space first if requested, but do not fail the whole operation on space failure
            if add_space {
                if let Err(e) = enigo.text(" ") {
                    eprintln!("⚠️ Failed to type leading space on attempt {}: {}", attempt + 1, e);
                }
            }

            // Type the main text
            if !text.is_empty() {
                match enigo.text(text) {
                    Ok(()) => {
                        println!("✅ enigo.text() OK on attempt {}", attempt + 1);
                        return true;
                    }
                    Err(e) => {
                        eprintln!("❌ enigo.text() failed on attempt {}: {}", attempt + 1, e);
                    }
                }
            } else {
                // No text to type, space (if any) already attempted
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
            static NEXT_OP_ID: AtomicU64 = AtomicU64::new(1);
            let op_id = NEXT_OP_ID.fetch_add(1, Ordering::Relaxed);
            let text_len = text.len();
            println!("📨 queue_typing op_id={}, len={}, add_space={}", op_id, text_len, add_space);
            sender
                .send(TypingCommand::Type { op_id, text, add_space })
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
        
        // Type with error handling; do not fail entire operation if space fails
        if add_space {
            if let Err(e) = enigo.text(" ") {
                eprintln!("⚠️ Failed to type leading space: {}", e);
            }
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
        // Only the owner (with a worker_handle) should shut down the worker.
        if self.worker_handle.is_some() {
            if let Some(sender) = self.sender.take() {
                let _ = sender.send(TypingCommand::Shutdown);
            }
            if let Some(handle) = self.worker_handle.take() {
                let _ = handle.join();
            }
            println!("🧵 Typing worker stopped by owner drop");
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
            
            match enigo.text("Hello from Typeswift diagnostic test!") {
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
