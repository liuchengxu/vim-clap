use {
    crate::fileworks::ByteLines,
    std::{
        mem,
        sync::atomic::{AtomicBool, Ordering},
    },
};

// Okay, let's try to do it again. Now a bit simpler.
// All threads are spawned from main. Then main thread hangs on receiver until every sender dies.
// All inner arrays are sent with mpsc when full or at the very end. All errors are now `io::Error`.

#[repr(transparent)]
pub struct KillUs(AtomicBool);

impl AsRef<KillUs> for KillUs {
    #[inline]
    fn as_ref(&self) -> &KillUs {
        self
    }
}

impl KillUs {
    #[inline]
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    #[inline]
    pub fn kill(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[inline]
    pub fn mustdie(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[inline]
pub fn spawn_me<P, T, L, N>(
    files: impl Iterator<Item = P>,
    sender: flume::Sender<Vec<T>>,
    capnum: usize,
    match_and_score: L,
    needle: N,
) where
    P: AsRef<[u8]>,
    L: Fn(&[u8], &N) -> Option<T>,
{
    let mut inner = Vec::with_capacity(capnum);

    files.for_each(|filebuf| {
        ByteLines::new(filebuf.as_ref()).for_each(|line| {
            if let Some(t) = match_and_score(line, &needle) {
                if inner.len() == inner.capacity() {
                    let msg = mem::replace(&mut inner, Vec::with_capacity(capnum));

                    let _any_result = sender.send(msg);
                }

                inner.push(t);
            }
        });
    });

    // Whatever is is, we will return errors anyway.
    let _any_result = sender.send(inner);
}
