use std::{
    any::Any,
    fs, io,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    thread,
};

//XXX N.B.
//XXX Actually, this iterator should be `FusedIterator`, but humans sometimes
//XXX forget to write `impl FusedIterator for Iter {}`. Let's assume this iterator is fused.
//XXX If it is not, this could lead to logic bugs, but not UB, so let's assume the iter you use is fused.
//XXX
//XXX If there's some strange results, feed `iter.fuse()` into this.

/// A tuple of vector with search results and total number of such results found.
///
/// The vector could have length lesser than total number of results.
/// `mwpuple.0` - vector, `mwpuple.1` - total, shown as usize.
pub type MWPuple<S> = (Vec<S>, usize);

/// Just `MWPuple` behind a mutex.
pub type MWPutex<S> = Mutex<MWPuple<S>>;

pub struct ThreadMe<I, P, E, S, F, G, O>
where
    I: Iterator<Item = Result<P, E>> + Send + 'static,
    P: AsRef<Path> + 'static,
    E: From<io::Error> + Send + 'static,
    S: Send + 'static,
    F: Fn(Vec<u8>, &mut Vec<S>, &MWPutex<S>) + Send + Sync + 'static,
    G: Fn(&MWPutex<S>, &mut Vec<S>, O) + Send + Sync + 'static,
    O: Fn(&[S], usize) + Send + Sync + 'static + Copy,
{
    number: usize,
    iter: Mutex<I>,
    shared: MWPutex<S>,
    do_text_into_inner: F,
    do_shared_and_inner: G,
    printer: O,
}

impl<I, P, E, S, F, G, O> ThreadMe<I, P, E, S, F, G, O>
where
    I: Iterator<Item = Result<P, E>> + Send + 'static,
    P: AsRef<Path> + 'static,
    E: From<io::Error> + Send + 'static,
    S: Send + 'static,
    F: Fn(Vec<u8>, &mut Vec<S>, &MWPutex<S>) + Send + Sync + 'static,
    G: Fn(&MWPutex<S>, &mut Vec<S>, O) + Send + Sync + 'static,
    O: Fn(&[S], usize) + Send + Sync + 'static + Copy,
{
    #[inline]
    pub fn new(
        number: usize,
        iter: Mutex<I>,
        shared: MWPutex<S>,
        do_text_into_inner: F,
        do_shared_and_inner: G,
        printer: O,
    ) -> Self {
        Self {
            number,
            iter,
            shared,
            do_text_into_inner,
            do_shared_and_inner,
            printer,
        }
    }

    #[inline]
    pub fn into_shared(self) -> MWPuple<S> {
        match self.shared.into_inner() {
            Ok(v) => v,
            Err(pois) => pois.into_inner(),
        }
    }
}

impl<I, P, E, S, F, G, O> Threader for ThreadMe<I, P, E, S, F, G, O>
where
    I: Iterator<Item = Result<P, E>> + Send + 'static,
    P: AsRef<Path> + 'static,
    E: From<io::Error> + Send + 'static,
    S: Send + 'static,
    F: Fn(Vec<u8>, &mut Vec<S>, &MWPutex<S>) + Send + Sync + 'static,
    G: Fn(&MWPutex<S>, &mut Vec<S>, O) + Send + Sync + 'static,
    O: Fn(&[S], usize) + Send + Sync + 'static + Copy,
{
    type Iter = I;
    type AsPath = P;
    type Error = E;
    type Score = S;

    #[inline]
    fn lock_fileiter(&self) -> MutexGuard<Self::Iter> {
        match self.iter.lock() {
            Ok(g) => g,
            Err(pois) => pois.into_inner(),
        }
    }

    #[inline]
    fn set_inner(&self) -> Vec<Self::Score> {
        Vec::with_capacity(self.number)
    }

    #[inline]
    fn do_the_thing(
        &self,
        text_of_file: Vec<u8>,
        inner: &mut Vec<Self::Score>,
        shared: &MWPutex<Self::Score>,
    ) {
        (self.do_text_into_inner)(text_of_file, inner, shared)
    }

    #[inline]
    fn filebuf_ended(&self, inner: &mut Vec<Self::Score>) {
        (self.do_shared_and_inner)(&self.shared, inner, self.printer)
    }

    #[inline]
    fn share(&self) -> &MWPutex<Self::Score> {
        &self.shared
    }
}

const FBUF_LEN: usize = 4;

pub trait Threader: 'static + Send + Sync {
    type Iter: Iterator<Item = Result<Self::AsPath, Self::Error>>;

    type AsPath: AsRef<Path>;

    type Error: 'static + From<io::Error> + Send;

    type Score;

    #[inline]
    fn run_chain(only_clone_me_work_with_ref: Arc<Self>, threadcount: u8) -> Vec<Self::Error> {
        let worker = only_clone_me_work_with_ref.deref();
        let mut unhandled_errors: Vec<Self::Error> = Vec::new();
        let handle_error = &mut { |e| unhandled_errors.push(e) };

        let filebuffer: &mut [Option<Self::AsPath>; FBUF_LEN] = &mut {
            // Currently this: `[None; FBUF_LEN];` can't be done,
            // so here's that.
            [None, None, None, None]
        };

        worker.fill_filebuf(filebuffer, handle_error);

        // If there's no more threadcount, we don't spawn bonus thread.
        // If there's no more items in iterator, we don't spawn bonus thread.
        let next_thread = if threadcount != 0 && filebuffer[FBUF_LEN - 1].is_some() {
            let cloned_self = Arc::clone(&only_clone_me_work_with_ref);

            let th = thread::spawn(move || Threader::run_chain(cloned_self, threadcount - 1));
            Some(th)
        } else {
            None
        };

        let mut inner_store = worker.set_inner();

        'outer: loop {
            for fbuf_idx in 0..FBUF_LEN {
                if let Some(file) = filebuffer[fbuf_idx].take() {
                    match fs::read(file) {
                        Ok(bytevec) => {
                            worker.do_the_thing(bytevec, &mut inner_store, worker.share())
                        }

                        // That's either not a file or there's some error.
                        Err(e) => handle_error(Self::Error::from(e)),
                    }
                } else {
                    // There's no more work to do, end the loop.
                    break 'outer;
                }
            }
            // Current filebuffer ended, fill it again.
            worker.filebuf_ended(&mut inner_store);
            worker.fill_filebuf(filebuffer, handle_error);
        }
        // Do this after loop once too.
        worker.filebuf_ended(&mut inner_store);

        // This thread's job is ended, join the created thread if any.
        if let Some(handle) = next_thread {
            let mut nthread_errors = match handle.join() {
                Ok(v) => v,
                Err(panic_data) => worker.handle_thread_panic(panic_data),
            };

            if nthread_errors.capacity() - nthread_errors.len()
                > unhandled_errors.capacity() - unhandled_errors.len()
            {
                nthread_errors.append(unhandled_errors.as_mut());
                nthread_errors
            } else {
                unhandled_errors.append(nthread_errors.as_mut());
                unhandled_errors
            }
        } else {
            unhandled_errors
        }
    }

    #[inline]
    fn fill_filebuf<F>(&self, buf: &mut [Option<Self::AsPath>; FBUF_LEN], handle_error: &mut F)
    where
        F: FnMut(Self::Error),
    {
        let mut lock_iter = self.lock_fileiter();
        let iter = lock_iter.deref_mut();

        for i in 0..FBUF_LEN {
            let mut opt_path = None;
            loop {
                match iter.next() {
                    Some(Ok(path)) => {
                        opt_path = Some(path);
                        break;
                    }
                    Some(Err(e)) => handle_error(e),
                    None => break,
                }
            }

            buf[i] = opt_path;
        }

        drop(lock_iter);
    }

    /// All this thing is about filtering lines in file, you do remember, right?
    fn do_the_thing(
        &self,
        text_of_file: Vec<u8>,
        inner: &mut Vec<Self::Score>,
        shared: &MWPutex<Self::Score>,
    );

    /// The threader will refill buffer automatically,
    /// but this function is called each time it ends,
    /// so some additional logic could be evoked at the end of each buffer.
    fn filebuf_ended(&self, inner: &mut Vec<Self::Score>);

    /// Pretty simple.
    fn lock_fileiter(&self) -> MutexGuard<Self::Iter>;

    fn share(&self) -> &MWPutex<Self::Score>;

    /// Creates the inner buffer for results.
    #[inline]
    fn set_inner(&self) -> Vec<Self::Score> {
        Vec::new()
    }

    /// If you want to do some other thing when one of the threads panic,
    /// you can write its logic there.
    #[cold]
    fn handle_thread_panic(&self, panic_data: Box<dyn Any + Send + 'static>) -> Vec<Self::Error> {
        eprintln!("One of the threads had thrown panic.\nThat should not had happen, so here's the data:\n{:?}", panic_data);

        vec![Self::Error::from(io::Error::new(
            std::io::ErrorKind::Other,
            "One of the threads throwed panic.",
        ))]
    }
}
