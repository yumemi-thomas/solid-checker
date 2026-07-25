use typefacts::{Cancellation, Session, SessionError};

#[test]
fn cancellation_is_part_of_the_public_session_api() {
    fn assert_clone<T: Clone>() {}
    fn assert_handle(session: &Session) -> Option<Cancellation> {
        session.cancellation_handle()
    }
    fn assert_result(cancellation: &Cancellation) -> Result<bool, SessionError> {
        cancellation.cancel_active()
    }

    assert_clone::<Cancellation>();
    let _ = assert_handle;
    let _ = assert_result;
}
