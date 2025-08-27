use crate::error::{VoicyError, VoicyResult};
use enigo::{Enigo, Keyboard, Settings};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone)]
pub struct TypingRequest {
    pub text: String,
    pub add_space: bool,
}

pub struct TypingQueue {
    sender: Sender<TypingRequest>,
    receiver: Arc<Mutex<Receiver<TypingRequest>>>,
    use_direct_execution: bool,
}

impl TypingQueue {
    pub fn new(use_direct_execution: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            use_direct_execution,
        }
    }

    pub fn queue_typing(&self, text: String, add_space: bool) -> VoicyResult<()> {
        if text.is_empty() && !add_space {
            return Ok(());
        }

        if self.use_direct_execution {
            self.execute_direct_typing(text, add_space)
        } else {
            self.queue_for_main_thread(text, add_space)
        }
    }

    fn execute_direct_typing(&self, text: String, add_space: bool) -> VoicyResult<()> {
        println!("💬 Direct typing execution: \"{}\"", text);
        
        thread::spawn(move || {
            match Enigo::new(&Settings::default()) {
                Ok(mut enigo) => {
                    if add_space {
                        if let Err(e) = enigo.text(" ") {
                            eprintln!("❌ Failed to type space: {}", e);
                            return;
                        }
                    }
                    
                    if !text.is_empty() {
                        match enigo.text(&text) {
                            Ok(()) => {
                                println!("✅ Successfully typed: \"{}\"", text);
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to type \"{}\": {}", text, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to create Enigo: {}", e);
                }
            }
        });
        
        Ok(())
    }

    fn queue_for_main_thread(&self, text: String, add_space: bool) -> VoicyResult<()> {
        let request = TypingRequest { text, add_space };
        
        self.sender.send(request).map_err(|e| {
            VoicyError::WindowOperationFailed(format!("Failed to queue typing request: {}", e))
        })?;
        
        Ok(())
    }

    pub fn process_queue(&self) -> VoicyResult<usize> {
        if self.use_direct_execution {
            return Ok(0);
        }
        
        let mut processed = 0;
        
        let receiver_guard = self.receiver.lock().map_err(|e| {
            VoicyError::WindowOperationFailed(format!("Failed to lock typing queue receiver: {}", e))
        })?;

        let mut requests = Vec::new();
        while let Ok(request) = receiver_guard.try_recv() {
            requests.push(request);
        }
        drop(receiver_guard);

        if !requests.is_empty() {
            println!("🔤 Processing {} typing requests on main thread", requests.len());
            
            let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
                VoicyError::WindowOperationFailed(format!("Failed to create Enigo on main thread: {}", e))
            })?;
            
            for request in requests {
                if self.execute_typing_request(&mut enigo, &request)? {
                    processed += 1;
                }
            }
        }

        Ok(processed)
    }

    fn execute_typing_request(&self, enigo: &mut Enigo, request: &TypingRequest) -> VoicyResult<bool> {
        if request.add_space {
            enigo.text(" ").map_err(|e| {
                VoicyError::WindowOperationFailed(format!("Failed to type space: {}", e))
            })?;
        }

        if !request.text.is_empty() {
            enigo.text(&request.text).map_err(|e| {
                VoicyError::WindowOperationFailed(format!("Failed to type \"{}\": {}", request.text, e))
            })?;
            
            println!("💬 Typed: \"{}\"", request.text);
        }

        Ok(true)
    }

    pub fn initialize_on_main_thread(&self) -> VoicyResult<()> {
        if self.use_direct_execution {
            println!("✅ Typing queue using direct execution mode");
            return Ok(());
        }
        
        match Enigo::new(&Settings::default()) {
            Ok(_) => {
                println!("✅ Typing queue initialized on main thread");
                Ok(())
            }
            Err(e) => {
                Err(VoicyError::WindowOperationFailed(
                    format!("Failed to initialize Enigo on main thread: {}", e)
                ))
            }
        }
    }

    pub fn get_sender(&self) -> Sender<TypingRequest> {
        self.sender.clone()
    }
}

impl Clone for TypingQueue {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
            use_direct_execution: self.use_direct_execution,
        }
    }
}

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
                thread::sleep(std::time::Duration::from_secs(1));
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
            thread::sleep(std::time::Duration::from_millis(500));
            
            let test_chars = ['T', 'e', 's', 't'];
            for ch in test_chars {
                match enigo.key(enigo::Key::Unicode(ch), enigo::Direction::Click) {
                    Ok(()) => println!("   ✅ Key '{}' sent successfully", ch),
                    Err(e) => println!("   ❌ Key '{}' failed: {}", ch, e),
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
            
        }
        Err(e) => {
            println!("   ❌ Failed to initialize Enigo: {}", e);
        }
    }
    
    println!("\n4. Testing alternative Enigo settings...");
    
    let custom_settings = Settings {
        release_keys_when_dropped: true,
        ..Settings::default()
    };
    
    match Enigo::new(&custom_settings) {
        Ok(mut enigo) => {
            println!("   ✅ Custom settings Enigo initialized");
            match enigo.text(" (custom settings)") {
                Ok(()) => println!("   ✅ Custom settings typing succeeded"),
                Err(e) => println!("   ❌ Custom settings typing failed: {}", e),
            }
        }
        Err(e) => {
            println!("   ❌ Custom settings Enigo failed: {}", e);
        }
    }
    
    println!("\n5. System Information:");
    println!("   📱 Platform: macOS");
    println!("   🔒 Accessibility permissions required for typing to other apps");
    println!("   ⚙️  To grant permissions:");
    println!("      1. Open System Preferences → Security & Privacy");
    println!("      2. Click 'Privacy' tab");
    println!("      3. Select 'Accessibility' in the left sidebar");
    println!("      4. Click the lock icon to make changes");
    println!("      5. Click '+' and add the Voicy application");
    println!("      6. Make sure Voicy is checked");
    
    println!("\n✅ Diagnostic complete!");
    println!("   If typing didn't work, the most likely cause is missing macOS accessibility permissions.");
}