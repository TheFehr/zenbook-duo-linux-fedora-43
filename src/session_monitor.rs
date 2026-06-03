use std::time::Duration;
use futures::StreamExt;
use zbus::{Connection, MatchRule, MessageStream, message::Type as MessageType};
use crate::config::Config;
use crate::usb::check_initial_state;
use crate::monitor_handling::handle_if_changed;
use log::{info, error};

pub async fn monitor_session_unlock(config: Config) {
    let connection = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to system D-Bus: {}", e);
            return;
        }
    };

    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface("org.freedesktop.login1.Session").unwrap()
        .member("Unlock").unwrap()
        .build();

    let mut stream = match MessageStream::for_match_rule(rule, &connection, Some(64)).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to subscribe to session unlock signal: {}", e);
            return;
        }
    };

    info!("Monitoring session unlock events...");

    while let Some(msg) = stream.next().await {
        if msg.is_ok() {
            info!("Session unlocked, re-applying monitor configuration...");
            for attempt in 1..=5 {
                tokio::time::sleep(Duration::from_millis(2000)).await;
                let (current_state, _) = check_initial_state(&config);
                if handle_if_changed(&current_state, &None, &config) {
                    info!("Monitor configuration re-applied successfully.");
                    break;
                }
                if attempt < 5 {
                    info!("Desktop environment not ready yet, retrying ({}/5)...", attempt);
                }
            }
        }
    }
}
