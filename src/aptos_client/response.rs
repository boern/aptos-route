#[derive(Debug)]
pub struct Response<T> {
    inner: T,
    // state: State,
}

impl<T> Response<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn and_then<U, E, F>(self, f: F) -> Result<Response<U>, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        // let (inner, state) = self.into_parts();
        let inner = self.into_inner();
        match f(inner) {
            Ok(new_inner) => Ok(Response::new(new_inner)),
            Err(err) => Err(err),
        }
    }

    pub fn map<U, F>(self, f: F) -> Response<U>
    where
        F: FnOnce(T) -> U,
    {
        // let (inner, state) = self.into_parts();
        let inner = self.into_inner();
        // Response::new(f(inner), state)
        Response::new(f(inner))
    }
}
