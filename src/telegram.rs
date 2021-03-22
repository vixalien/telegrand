use grammers_client::{Client, Config, InputMessage, SignInError, Update};
use grammers_client::client::chats::AuthorizationError;
use grammers_client::client::messages::MessageIter;
use grammers_client::types::{Dialog, LoginToken, Message};
use grammers_session::FileSession;
use gtk::glib;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::sync::{Arc, Mutex};
use tokio::{runtime, task};
use tokio::sync::mpsc;

use crate::config;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub enum TelegramEvent {
    AccountAuthorized,
    AccountNotAuthorized,
    NeedConfirmationCode,
    PhoneNumberError(AuthorizationError),
    ConfirmationCodeError(SignInError),

    RequestedDialog(Dialog, MessageIter),
    RequestedMessage(Message),
    NewMessage(Message),
}

pub enum GtkEvent {
    SendPhoneNumber(String),
    SendConfirmationCode(String),

    RequestDialogs,
    RequestNextMessages(Arc<Mutex<MessageIter>>),
    SendMessage(Arc<Dialog>, InputMessage),
}

pub fn spawn(tg_sender: glib::Sender<TelegramEvent>, gtk_receiver: mpsc::Receiver<GtkEvent>) {
    SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .init()
        .unwrap();

    std::thread::spawn(move || {
        let result = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(start(tg_sender, gtk_receiver));

        // Panic on error
        // TODO: add automatic reconnection on error
        result.expect("Telegram thread error")
    });
}

pub fn send_gtk_event(gtk_sender: &mpsc::Sender<GtkEvent>, event: GtkEvent) {
    let _ = runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(gtk_sender.send(event));
}

async fn start(tg_sender: glib::Sender<TelegramEvent>, mut gtk_receiver: mpsc::Receiver<GtkEvent>) -> Result {
    let api_id = config::TG_API_ID.to_owned();
    let api_hash = config::TG_API_HASH.to_owned();

    let mut client = Client::connect(Config {
        session: FileSession::load_or_create("telegrand.session")?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;

    if !client.is_authorized().await? {
        tg_sender.send(TelegramEvent::AccountNotAuthorized).unwrap();

        let mut token: Option<LoginToken> = None;
        while let Some(event) = gtk_receiver.recv().await {
            match event {
                GtkEvent::SendPhoneNumber(number) => {
                    match client.request_login_code(&number, api_id, &api_hash).await {
                        Ok(token_) => {
                            token = Some(token_);
                            tg_sender.send(TelegramEvent::NeedConfirmationCode).unwrap();
                        }
                        Err(error) => {
                            tg_sender.send(TelegramEvent::PhoneNumberError(error)).unwrap();
                        }
                    };
                }
                GtkEvent::SendConfirmationCode(code) => {
                    match client.sign_in(token.as_ref().unwrap(), &code).await {
                        Ok(_) => {
                            // TODO: sign out when closing the app if this fails
                            client.session().save()?;
                            break;
                        }
                        Err(error) => {
                            tg_sender.send(TelegramEvent::ConfirmationCodeError(error)).unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    tg_sender.send(TelegramEvent::AccountAuthorized).unwrap();

    let mut client_handle = client.handle();
    let tg_sender_clone = tg_sender.clone();
    task::spawn(async move {
        while let Some(updates) = client.next_updates().await.unwrap() {
            for update in updates {
                match update {
                    Update::NewMessage(message) => {
                        tg_sender_clone.send(TelegramEvent::NewMessage(message)).unwrap();
                    }
                    _ => {}
                }
            }
        }
    });

    while let Some(event) = gtk_receiver.recv().await {
        match event {
            GtkEvent::RequestDialogs => {
                let mut dialogs = client_handle.iter_dialogs();
                while let Some(dialog) = dialogs.next().await.unwrap() {
                    let iterator = client_handle.iter_messages(dialog.chat());
                    tg_sender.send(TelegramEvent::RequestedDialog(dialog, iterator)).unwrap();
                }
            }
            GtkEvent::RequestNextMessages(iterator) => {
                // Return the next 20 messages
                let mut iterator = iterator.lock().unwrap();
                for _ in 0..20 {
                    if let Some(message) = iterator.next().await.unwrap() {
                        tg_sender.send(TelegramEvent::RequestedMessage(message)).unwrap();
                    }
                }
            }
            GtkEvent::SendMessage(dialog, message) => {
                client_handle.send_message(dialog.chat(), message).await?;
            }
            _ => {}
        }
    }

    Ok(())
}
