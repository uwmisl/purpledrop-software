use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use crate::protobuf::PurpleDropEvent as EventMessage;

type HandlersArray = Arc<Mutex<Vec<Box<dyn FnMut(EventMessage) ->() + Send>>>>;

pub struct EventBroker {
    chan_in: Sender<Box<EventMessage>>,
    handlers: HandlersArray,
}

/// A clearing house for published events
/// 
/// Captures events from whereever they are generated, and pushes them out
/// to any registered handlers. Handlers may, e.g., log to a file or push
/// via websocket to a client.
///
/// The events themselves are structs generated by `prost` from google 
/// protobuf message definitions.
/// 
impl EventBroker {
    pub fn add_handler<F: 'static>(&mut self, handler: F)
        where F: FnMut(EventMessage) -> () + Send,
    {
        let mut handlers = self.handlers.lock().unwrap();

        handlers.push(Box::new(handler));
    }

    pub fn send(&mut self, msg: EventMessage) {
        let mut handlers = self.handlers.lock().unwrap();
        for h in &mut *handlers {
            h(msg.clone());
        }
    }

    pub fn new() -> EventBroker {
        let (chan_in, chan_out) = channel::<Box<EventMessage>>();
        let handlers: HandlersArray = Arc::new(Mutex::new(Vec::new()));
        // Create a clone for the thread closure
        let thread_handlers = handlers.clone();
        let thread = thread::spawn(move || {
            loop {
                let event = chan_out.recv().unwrap();
                let mut handlers = thread_handlers.lock().unwrap();
                for h in &mut *handlers {
                    h(*event.clone());
                }
                println!("Got an event")
            }
        });
        EventBroker{chan_in, handlers: handlers.clone()}
    }
}

#[cfg(test)]
mod tests {

    use crate::eventbroker::*;
    use crate::protobuf::*;
    #[test]
    fn test_event_broker() {
        let mut broker = EventBroker::new();

        // Setup handler to push messages into a channel for the test
        let (cin, cout) = channel::<EventMessage>();
        broker.add_handler(move |msg| {
            cin.send(msg).unwrap();
        });

        // Create and send a test message
        let msg = PurpleDropEvent{
            msg: Some(purple_drop_event::Msg::Settings(
                    Settings{frequency: 10.0, timestamp: Some(Timestamp{seconds: 100, nanos: 11}) },
            ))
        };
        broker.send(msg);

        // Check that we recieve the expected message
        let last_msg = cout.recv().unwrap();
        match last_msg.msg.unwrap() {
            purple_drop_event::Msg::Settings(s) => assert_eq!(s.timestamp.unwrap().seconds, 100),
            _ => panic!("Got unexpected message"),
        }
    }
}