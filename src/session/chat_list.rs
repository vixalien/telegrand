use crate::session::Chat;
use crate::utils::do_async;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use tdgrand::{enums, functions};

mod imp {
    use super::*;
    use indexmap::IndexMap;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    pub struct ChatList {
        pub list: RefCell<IndexMap<i64, Chat>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatList {
        const NAME: &'static str = "ChatList";
        type Type = super::ChatList;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ChatList {}

    impl ListModelImpl for ChatList {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            Chat::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .values()
                .nth(position as usize)
                .map(glib::object::Cast::upcast_ref::<glib::Object>)
                .cloned()
        }
    }
}

glib::wrapper! {
    pub struct ChatList(ObjectSubclass<imp::ChatList>)
        @implements gio::ListModel;
}

impl Default for ChatList {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatList {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create ChatList")
    }

    fn items_added(&self, added: usize) {
        let priv_ = imp::ChatList::from_instance(self);
        let list = priv_.list.borrow();
        let position = list.len() - added;
        self.items_changed(position as u32, 0, added as u32);
    }

    pub fn load(&self, client_id: i32) {
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            async move {
                functions::get_chats(client_id, tdgrand::enums::ChatList::Main, format!("{}", i64::MAX), 0, i32::MAX).await
            },
            clone!(@weak self as obj => move |result| async move {
                if let Ok(enums::Chats::Chats(chats)) = result {
                    let added = chats.chat_ids.len();

                    {
                        let priv_ = imp::ChatList::from_instance(&obj);
                        let mut list = priv_.list.borrow_mut();

                        for chat_id in chats.chat_ids {
                            list.insert(chat_id, Chat::new(chat_id, client_id));
                        }
                    }

                    obj.items_added(added);
                }
            }),
        );
    }
}
