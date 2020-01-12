use tokio::sync::oneshot::{channel, Sender, Receiver};
use std::future::Future;
use std::pin::Pin;
use std::task::{Poll, Context};
use atomic_take::AtomicTake;

pub fn spawn<F>(future: F) -> KillHandle
where
    F: Future<Output = ()> + Send + 'static,
{
    let (handle, fut) = new_handle(future);
    tokio::spawn(fut);
    handle
}

pub fn new_handle<F: Future>(future: F) -> (KillHandle, Killable<F>) {
    let (send, recv) = channel();
    let fut = Killable {
        inner: Some(future),
        kill: recv,
    };
    (KillHandle { inner: AtomicTake::new(send) }, fut)
}

#[derive(Debug)]
pub struct KillSpawn {
    recv: Receiver<()>,
}
impl KillSpawn {
    pub fn new() -> (KillSpawn, KillHandle) {
        let (send, recv) = channel();
        (KillSpawn { recv }, KillHandle { inner: AtomicTake::new(send) })
    }
    pub fn into_killable<F>(self, future: F) -> Killable<F> {
        Killable {
            inner: Some(future),
            kill: self.recv,
        }
    }
    pub fn spawn<F: Future<Output = ()> + Send + 'static>(self, future: F) {
        tokio::spawn(self.into_killable(future));
    }
}

#[derive(Debug)]
pub struct KillHandle {
    inner: AtomicTake<Sender<()>>,
}
impl KillHandle {
    pub fn kill(&self) {
        if let Some(chan) = self.inner.take() {
            let _ = chan.send(());
        }
    }
}
impl Drop for KillHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

#[derive(Debug)]
pub struct Killable<F> {
    inner: Option<F>,
    kill: Receiver<()>,
}
impl<F: Future> Future for Killable<F> {
    type Output = Option<F::Output>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let this = Pin::into_inner_unchecked(self);
            if this.inner.is_none() {
                return Poll::Ready(None);
            }
            let kill_pin = Pin::new_unchecked(&mut this.kill);
            match Future::poll(kill_pin, &mut *ctx) {
                Poll::Ready(_) => {
                    this.inner = None;
                    return Poll::Ready(None);
                },
                Poll::Pending => {},
            }
            let inner_pin = Pin::new_unchecked(this.inner.as_mut().unwrap());
            match Future::poll(inner_pin, ctx) {
                Poll::Ready(val) => {
                    this.inner = None;
                    Poll::Ready(Some(val))
                },
                Poll::Pending => Poll::Pending,
            }
        }
    }
}
