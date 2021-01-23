use std::collections::VecDeque;
use std::pin::Pin;

use futures::stream::Stream;
use futures::task::{
    Context,
    Waker,
    Poll,
};

/* (1) Adding spot is limited unless you use heap allocation (Arc<Mutex<>>);
 * (2) Next may be spread across mutliple tasks;
 *      -> make sure waking up all of those tasks;
 *      -> take care of the order of wake();
 *      -> streams wake up only one task where it had been polled lately;
 */
pub struct Mux<S>
    where S: Stream,
{
    streams: VecDeque<Pin<Box<S>>>,
    waker: Vec<Waker>,
}

impl<S> Mux<S>
    where S: Stream,
{
    pub fn new() -> Self {
        Mux::<S> {
            streams: VecDeque::new(),
            waker: Vec::new(),
        }
    }

    pub fn add(self: &mut Self, stream: Pin<Box<S>>) {
        self.streams.push_back(stream);

        while let Some(waker) = self.waker.pop() {
            waker.wake();
        }
    }
}

impl<S> Stream for Mux<S>
    where S: Stream,
{
    type Item = <S as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut none = true;
        let mut pendings = VecDeque::new();

        while let Some(mut boxed) = self.streams.pop_front() {
            let pinned = boxed.as_mut();
            match pinned.poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    self.streams.append(&mut pendings);
                    self.streams.push_back(boxed);
                    return Poll::Ready(Some(item));
                },
                Poll::Ready(None) => {
                    continue;
                },
                Poll::Pending => {
                    none = false;
                    pendings.push_back(boxed);
                    continue;
                },
            }
        }

        self.streams.append(&mut pendings);
        
        if none {
            Poll::Ready(None)
        } else {
            self.waker.push(cx.waker().clone());
            Poll::Pending
        }
    }
}


pub struct MuxDyn<T> {
    streams: VecDeque<Pin<Box<dyn Stream<Item = T>>>>,
    waker: Vec<Waker>,
}

impl<T> MuxDyn<T> {
    pub fn new() -> Self {
        MuxDyn::<T> {
            streams: VecDeque::new(),
            waker: Vec::new(),
        }
    }

    pub fn add(self: &mut Self, stream: Pin<Box<dyn Stream<Item = T>>>) {
        self.streams.push_back(stream);
        while let Some(waker) = self.waker.pop() {
            waker.wake();
        }
    }
}

impl<T> Stream for MuxDyn<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut none = true;
        let mut pendings = VecDeque::new();

        while let Some(mut boxed) = self.streams.pop_front() {
            let pinned = boxed.as_mut();
            match pinned.poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    self.streams.append(&mut pendings);
                    self.streams.push_back(boxed);
                    return Poll::Ready(Some(item));
                },
                Poll::Ready(None) => {
                    continue;
                },
                Poll::Pending => {
                    none = false;
                    pendings.push_back(boxed);
                    continue;
                },
            }
        }

        self.streams.append(&mut pendings);

        if none {
            Poll::Ready(None)
        } else {
            self.waker.push(cx.waker().clone());
            Poll::Pending
        }
    }
}
