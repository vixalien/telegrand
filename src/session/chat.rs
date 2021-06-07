use crate::utils::do_async;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::glib;
use tdgrand::{enums, functions};
use tdgrand::types::Chat as TelegramChat;

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default)]
    pub struct Chat {
        pub chat_id: Cell<i64>,
        pub client_id: Cell<i32>,
        pub telegram_chat: RefCell<Option<TelegramChat>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Chat {
        const NAME: &'static str = "Chat";
        type Type = super::Chat;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Chat {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_int64(
                        "chat-id",
                        "Chat Id",
                        "The id of this chat",
                        std::i64::MIN,
                        std::i64::MAX,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_int(
                        "client-id",
                        "Client Id",
                        "The client id",
                        std::i32::MIN,
                        std::i32::MAX,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_string(
                        "title",
                        "Title",
                        "The title of this chat",
                        None,
                        glib::ParamFlags::READABLE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "chat-id" => {
                    let chat_id = value.get().unwrap();
                    self.chat_id.set(chat_id);
                }
                "client-id" => {
                    let client_id = value.get().unwrap();
                    self.client_id.set(client_id);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "chat-id" => obj.chat_id().to_value(),
                "client-id" => obj.client_id().to_value(),
                "title" => obj.title().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.load_chat();
        }
    }
}

glib::wrapper! {
    pub struct Chat(ObjectSubclass<imp::Chat>);
}

impl Chat {
    pub fn new(chat_id: i64, client_id: i32) -> Self {
        glib::Object::new(&[("chat-id", &chat_id), ("client-id", &client_id)])
            .expect("Failed to create Chat")
    }

    fn chat_id(&self) -> i64 {
        let priv_ = imp::Chat::from_instance(self);
        priv_.chat_id.get()
    }

    fn client_id(&self) -> i32 {
        let priv_ = imp::Chat::from_instance(self);
        priv_.client_id.get()
    }

    fn telegram_chat(&self) -> Option<TelegramChat> {
        let priv_ = imp::Chat::from_instance(self);
        priv_.telegram_chat.borrow().to_owned()
    }

    fn set_telegram_chat(&self, telegram_chat: TelegramChat) {
        let priv_ = imp::Chat::from_instance(self);
        priv_.telegram_chat.replace(Some(telegram_chat));

        self.notify("title");
    }

    fn title(&self) -> String {
        self.telegram_chat().unwrap_or_default().title
    }

    fn load_chat(&self) {
        let client_id = self.client_id();
        let chat_id = self.chat_id();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::get_chat(client_id, chat_id).await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Ok(enums::Chat::Chat(chat)) = result {
                    obj.set_telegram_chat(chat);
                }
            }),
        );
    }
}
