pub trait AsRuntimeId<T> {
    fn runtime_id() -> &'static T;
}
