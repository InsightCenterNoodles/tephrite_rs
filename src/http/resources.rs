use bevy::{platform::collections::HashMap, prelude::*};

use http::{Request, Response};

pub type HandlerBox = Box<dyn HTTPNodeHandler>;

#[derive(Resource, Default)]
pub struct HTTPResources {
    path_lookup: HashMap<String, Box<dyn HTTPNodeHandler>>,
}

impl HTTPResources {
    pub(crate) fn find(&self, path: &str) -> Option<&HandlerBox> {
        self.path_lookup.get(path)
    }

    pub fn insert<T: HTTPNodeHandler + 'static>(
        &mut self,
        path: String,
        handler: T,
    ) -> Option<HandlerBox> {
        self.path_lookup.insert(path, Box::new(handler))
    }

    pub fn remove(&mut self, path: &str) -> Option<HandlerBox> {
        self.path_lookup.remove(path)
    }
}

pub trait HTTPNodeHandler: Sync + Send {
    #[allow(unused)]
    fn on_get(
        &self,
        world: &mut World,
        request: &Request<bytes::Bytes>,
    ) -> Option<Response<bytes::Bytes>> {
        None
    }

    #[allow(unused)]
    fn on_post(
        &self,
        world: &mut World,
        request: &Request<bytes::Bytes>,
    ) -> Option<Response<bytes::Bytes>> {
        None
    }
}
