use std::any::Any;

use crate::{Error, Result, renderers::Renderer};

/// Represents a piece of work done by the [`RendererWorker`].
///
/// Each task receives the worker's current param, or `None` if it has none. Returning `Some`
/// replaces the param for the following tasks, and returning `None` keeps the current one.
pub type RendererTask = fn(
    &dyn Renderer,
    Option<&mut dyn RendererTaskParam>,
) -> Result<Option<Box<dyn RendererTaskParam>>>;

/// Represents the data passed to the tasks of a [`RendererWorker`].
pub trait RendererTaskParam: Any + Send + Sync {}

/// Executes its tasks on a dedicated thread, using its own thread-local resources of the
/// [`Renderer`].
pub trait RendererWorker {
    /// Checks whether the worker is currently working.
    fn is_working(&self) -> bool;
    /// The worker starts executing `tasks` on its own thread, with `param` as the initial task
    /// param. If `once` is `true`, the tasks are executed a single time, otherwise they are
    /// repeated until [`finish`](RendererWorker::finish) is called. `tasks` cannot be empty. On
    /// failure, the error is returned along with `param`.
    fn work(
        &mut self,
        tasks: &[RendererTask],
        param: Option<Box<dyn RendererTaskParam>>,
        once: bool,
    ) -> std::result::Result<(), (Error, Option<Box<dyn RendererTaskParam>>)>;
    /// The worker completes its remaining tasks and then ends the shift.
    fn finish(&mut self) -> Result<Option<Box<dyn RendererTaskParam>>>;
}

impl dyn RendererTaskParam + '_ {
    /// Downcasts the param to `T`. Returns an error if the param is not a `T`.
    pub fn downcast_ref<T: RendererTaskParam>(&self) -> Result<&T> {
        crate::renderers::downcast_ref::<T>(self)
    }

    /// Downcasts the param to `T`. Returns an error if the param is not a `T`.
    pub fn downcast_mut<T: RendererTaskParam>(&mut self) -> Result<&mut T> {
        crate::renderers::downcast_mut::<T>(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ParamA(usize);
    impl RendererTaskParam for ParamA {}

    struct ParamB;
    impl RendererTaskParam for ParamB {}

    #[test]
    fn test_task_param_downcast_ref() {
        let param: Box<dyn RendererTaskParam> = Box::new(ParamA(5));
        assert_eq!(param.downcast_ref::<ParamA>().unwrap().0, 5);

        let param: Box<dyn RendererTaskParam> = Box::new(ParamB);
        assert!(matches!(
            param.downcast_ref::<ParamA>(),
            Err(Error::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_task_param_downcast_mut() {
        let mut param: Box<dyn RendererTaskParam> = Box::new(ParamA(5));
        param.downcast_mut::<ParamA>().unwrap().0 = 10;
        assert_eq!(param.downcast_ref::<ParamA>().unwrap().0, 10);

        let mut param: Box<dyn RendererTaskParam> = Box::new(ParamB);
        assert!(matches!(
            param.downcast_mut::<ParamA>(),
            Err(Error::InvalidArgument(_))
        ));
    }
}
