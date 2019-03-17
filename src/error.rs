//! Error and Result module
use std::cell::RefCell;
use std::io::Error as IoError;
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use std::{fmt, io, result};

// use actix::MailboxError;
use actix_utils::timeout::TimeoutError;
use backtrace::Backtrace;
use cookie;
use derive_more::{Display, From};
use futures::Canceled;
use http::uri::InvalidUri;
use http::{header, Error as HttpError, StatusCode};
use httparse;
use serde::de::value::Error as DeError;
use serde_json::error::Error as JsonError;
use serde_urlencoded::ser::Error as FormError;
use tokio_timer::Error as TimerError;

// re-exports
pub use cookie::ParseError as CookieParseError;

use crate::body::Body;
use crate::response::Response;

/// A specialized [`Result`](https://doc.rust-lang.org/std/result/enum.Result.html)
/// for actix web operations
///
/// This typedef is generally used to avoid writing out
/// `actix_http::error::Error` directly and is otherwise a direct mapping to
/// `Result`.
pub type Result<T, E = Error> = result::Result<T, E>;

/// General purpose actix web error.
///
/// An actix web error is used to carry errors from `failure` or `std::error`
/// through actix in a convenient way.  It can be created through
/// converting errors with `into()`.
///
/// Whenever it is created from an external object a response error is created
/// for it that can be used to create an http response from it this means that
/// if you have access to an actix `Error` you can always get a
/// `ResponseError` reference from it.
pub struct Error {
    cause: Box<ResponseError>,
    backtrace: Option<Backtrace>,
}

impl Error {
    /// Returns the reference to the underlying `ResponseError`.
    pub fn as_response_error(&self) -> &ResponseError {
        self.cause.as_ref()
    }

    /// Returns a reference to the Backtrace carried by this error, if it
    /// carries one.
    ///
    /// This uses the same `Backtrace` type that `failure` uses.
    pub fn backtrace(&self) -> &Backtrace {
        if let Some(bt) = self.cause.backtrace() {
            bt
        } else {
            self.backtrace.as_ref().unwrap()
        }
    }

    /// Converts error to a response instance and set error message as response body
    pub fn response_with_message(self) -> Response {
        let message = format!("{}", self);
        let resp: Response = self.into();
        resp.set_body(Body::from(message))
    }
}

/// Error that can be converted to `Response`
pub trait ResponseError: fmt::Debug + fmt::Display {
    /// Create response for error
    ///
    /// Internal server error is generated by default.
    fn error_response(&self) -> Response {
        Response::new(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Response
    fn backtrace(&self) -> Option<&Backtrace> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.cause, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(bt) = self.cause.backtrace() {
            write!(f, "{:?}\n\n{:?}", &self.cause, bt)
        } else {
            write!(
                f,
                "{:?}\n\n{:?}",
                &self.cause,
                self.backtrace.as_ref().unwrap()
            )
        }
    }
}

/// Convert `Error` to a `Response` instance
impl From<Error> for Response {
    fn from(err: Error) -> Self {
        Response::from_error(err)
    }
}

/// `Error` for any error that implements `ResponseError`
impl<T: ResponseError + 'static> From<T> for Error {
    fn from(err: T) -> Error {
        let backtrace = if err.backtrace().is_none() {
            Some(Backtrace::new())
        } else {
            None
        };
        Error {
            cause: Box::new(err),
            backtrace,
        }
    }
}

/// Return `GATEWAY_TIMEOUT` for `TimeoutError`
impl<E: ResponseError> ResponseError for TimeoutError<E> {
    fn error_response(&self) -> Response {
        match self {
            TimeoutError::Service(e) => e.error_response(),
            TimeoutError::Timeout => Response::new(StatusCode::GATEWAY_TIMEOUT),
        }
    }
}

/// `InternalServerError` for `JsonError`
impl ResponseError for JsonError {}

/// `InternalServerError` for `FormError`
impl ResponseError for FormError {}

/// `InternalServerError` for `TimerError`
impl ResponseError for TimerError {}

/// Return `BAD_REQUEST` for `de::value::Error`
impl ResponseError for DeError {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

/// Return `BAD_REQUEST` for `Utf8Error`
impl ResponseError for Utf8Error {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

/// Return `InternalServerError` for `HttpError`,
/// Response generation can return `HttpError`, so it is internal error
impl ResponseError for HttpError {}

/// Return `InternalServerError` for `io::Error`
impl ResponseError for io::Error {
    fn error_response(&self) -> Response {
        match self.kind() {
            io::ErrorKind::NotFound => Response::new(StatusCode::NOT_FOUND),
            io::ErrorKind::PermissionDenied => Response::new(StatusCode::FORBIDDEN),
            _ => Response::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

/// `BadRequest` for `InvalidHeaderValue`
impl ResponseError for header::InvalidHeaderValue {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

/// `BadRequest` for `InvalidHeaderValue`
impl ResponseError for header::InvalidHeaderValueBytes {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

/// `InternalServerError` for `futures::Canceled`
impl ResponseError for Canceled {}

// /// `InternalServerError` for `actix::MailboxError`
// impl ResponseError for MailboxError {}

/// A set of errors that can occur during parsing HTTP streams
#[derive(Debug, Display)]
pub enum ParseError {
    /// An invalid `Method`, such as `GE.T`.
    #[display(fmt = "Invalid Method specified")]
    Method,
    /// An invalid `Uri`, such as `exam ple.domain`.
    #[display(fmt = "Uri error: {}", _0)]
    Uri(InvalidUri),
    /// An invalid `HttpVersion`, such as `HTP/1.1`
    #[display(fmt = "Invalid HTTP version specified")]
    Version,
    /// An invalid `Header`.
    #[display(fmt = "Invalid Header provided")]
    Header,
    /// A message head is too large to be reasonable.
    #[display(fmt = "Message head is too large")]
    TooLarge,
    /// A message reached EOF, but is not complete.
    #[display(fmt = "Message is incomplete")]
    Incomplete,
    /// An invalid `Status`, such as `1337 ELITE`.
    #[display(fmt = "Invalid Status provided")]
    Status,
    /// A timeout occurred waiting for an IO event.
    #[allow(dead_code)]
    #[display(fmt = "Timeout")]
    Timeout,
    /// An `io::Error` that occurred while trying to read or write to a network
    /// stream.
    #[display(fmt = "IO error: {}", _0)]
    Io(IoError),
    /// Parsing a field as string failed
    #[display(fmt = "UTF8 error: {}", _0)]
    Utf8(Utf8Error),
}

/// Return `BadRequest` for `ParseError`
impl ResponseError for ParseError {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

impl From<IoError> for ParseError {
    fn from(err: IoError) -> ParseError {
        ParseError::Io(err)
    }
}

impl From<InvalidUri> for ParseError {
    fn from(err: InvalidUri) -> ParseError {
        ParseError::Uri(err)
    }
}

impl From<Utf8Error> for ParseError {
    fn from(err: Utf8Error) -> ParseError {
        ParseError::Utf8(err)
    }
}

impl From<FromUtf8Error> for ParseError {
    fn from(err: FromUtf8Error) -> ParseError {
        ParseError::Utf8(err.utf8_error())
    }
}

impl From<httparse::Error> for ParseError {
    fn from(err: httparse::Error) -> ParseError {
        match err {
            httparse::Error::HeaderName
            | httparse::Error::HeaderValue
            | httparse::Error::NewLine
            | httparse::Error::Token => ParseError::Header,
            httparse::Error::Status => ParseError::Status,
            httparse::Error::TooManyHeaders => ParseError::TooLarge,
            httparse::Error::Version => ParseError::Version,
        }
    }
}

#[derive(Display, Debug, From)]
/// A set of errors that can occur during payload parsing
pub enum PayloadError {
    /// A payload reached EOF, but is not complete.
    #[display(fmt = "A payload reached EOF, but is not complete.")]
    Incomplete(Option<io::Error>),
    /// Content encoding stream corruption
    #[display(fmt = "Can not decode content-encoding.")]
    EncodingCorrupted,
    /// A payload reached size limit.
    #[display(fmt = "A payload reached size limit.")]
    Overflow,
    /// A payload length is unknown.
    #[display(fmt = "A payload length is unknown.")]
    UnknownLength,
    /// Http2 payload error
    #[display(fmt = "{}", _0)]
    Http2Payload(h2::Error),
}

impl From<io::Error> for PayloadError {
    fn from(err: io::Error) -> Self {
        PayloadError::Incomplete(Some(err))
    }
}

/// `PayloadError` returns two possible results:
///
/// - `Overflow` returns `PayloadTooLarge`
/// - Other errors returns `BadRequest`
impl ResponseError for PayloadError {
    fn error_response(&self) -> Response {
        match *self {
            PayloadError::Overflow => Response::new(StatusCode::PAYLOAD_TOO_LARGE),
            _ => Response::new(StatusCode::BAD_REQUEST),
        }
    }
}

/// Return `BadRequest` for `cookie::ParseError`
impl ResponseError for cookie::ParseError {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

#[derive(Debug, Display, From)]
/// A set of errors that can occur during dispatching http requests
pub enum DispatchError {
    /// Service error
    Service,

    /// An `io::Error` that occurred while trying to read or write to a network
    /// stream.
    #[display(fmt = "IO error: {}", _0)]
    Io(io::Error),

    /// Http request parse error.
    #[display(fmt = "Parse error: {}", _0)]
    Parse(ParseError),

    /// Http/2 error
    #[display(fmt = "{}", _0)]
    H2(h2::Error),

    /// The first request did not complete within the specified timeout.
    #[display(fmt = "The first request did not complete within the specified timeout")]
    SlowRequestTimeout,

    /// Disconnect timeout. Makes sense for ssl streams.
    #[display(fmt = "Connection shutdown timeout")]
    DisconnectTimeout,

    /// Payload is not consumed
    #[display(fmt = "Task is completed but request's payload is not consumed")]
    PayloadIsNotConsumed,

    /// Malformed request
    #[display(fmt = "Malformed request")]
    MalformedRequest,

    /// Internal error
    #[display(fmt = "Internal error")]
    InternalError,

    /// Unknown error
    #[display(fmt = "Unknown error")]
    Unknown,
}

/// A set of error that can occure during parsing content type
#[derive(PartialEq, Debug, Display)]
pub enum ContentTypeError {
    /// Can not parse content type
    #[display(fmt = "Can not parse content type")]
    ParseError,
    /// Unknown content encoding
    #[display(fmt = "Unknown content encoding")]
    UnknownEncoding,
}

/// Return `BadRequest` for `ContentTypeError`
impl ResponseError for ContentTypeError {
    fn error_response(&self) -> Response {
        Response::new(StatusCode::BAD_REQUEST)
    }
}

/// Helper type that can wrap any error and generate custom response.
///
/// In following example any `io::Error` will be converted into "BAD REQUEST"
/// response as opposite to *INTERNAL SERVER ERROR* which is defined by
/// default.
///
/// ```rust
/// # extern crate actix_http;
/// # use std::io;
/// # use actix_http::*;
///
/// fn index(req: Request) -> Result<&'static str> {
///     Err(error::ErrorBadRequest(io::Error::new(io::ErrorKind::Other, "error")))
/// }
/// # fn main() {}
/// ```
pub struct InternalError<T> {
    cause: T,
    status: InternalErrorType,
    backtrace: Backtrace,
}

enum InternalErrorType {
    Status(StatusCode),
    Response(RefCell<Option<Response>>),
}

impl<T> InternalError<T> {
    /// Create `InternalError` instance
    pub fn new(cause: T, status: StatusCode) -> Self {
        InternalError {
            cause,
            status: InternalErrorType::Status(status),
            backtrace: Backtrace::new(),
        }
    }

    /// Create `InternalError` with predefined `Response`.
    pub fn from_response(cause: T, response: Response) -> Self {
        InternalError {
            cause,
            status: InternalErrorType::Response(RefCell::new(Some(response))),
            backtrace: Backtrace::new(),
        }
    }
}

impl<T> fmt::Debug for InternalError<T>
where
    T: fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.cause, f)
    }
}

impl<T> fmt::Display for InternalError<T>
where
    T: fmt::Display + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.cause, f)
    }
}

impl<T> ResponseError for InternalError<T>
where
    T: fmt::Debug + fmt::Display + 'static,
{
    fn backtrace(&self) -> Option<&Backtrace> {
        Some(&self.backtrace)
    }

    fn error_response(&self) -> Response {
        match self.status {
            InternalErrorType::Status(st) => Response::new(st),
            InternalErrorType::Response(ref resp) => {
                if let Some(resp) = resp.borrow_mut().take() {
                    resp
                } else {
                    Response::new(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    }
}

/// Convert Response to a Error
impl From<Response> for Error {
    fn from(res: Response) -> Error {
        InternalError::from_response("", res).into()
    }
}

/// Helper function that creates wrapper of any error and generate *BAD
/// REQUEST* response.
#[allow(non_snake_case)]
pub fn ErrorBadRequest<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::BAD_REQUEST).into()
}

/// Helper function that creates wrapper of any error and generate
/// *UNAUTHORIZED* response.
#[allow(non_snake_case)]
pub fn ErrorUnauthorized<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::UNAUTHORIZED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *PAYMENT_REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorPaymentRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::PAYMENT_REQUIRED).into()
}

/// Helper function that creates wrapper of any error and generate *FORBIDDEN*
/// response.
#[allow(non_snake_case)]
pub fn ErrorForbidden<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::FORBIDDEN).into()
}

/// Helper function that creates wrapper of any error and generate *NOT FOUND*
/// response.
#[allow(non_snake_case)]
pub fn ErrorNotFound<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::NOT_FOUND).into()
}

/// Helper function that creates wrapper of any error and generate *METHOD NOT
/// ALLOWED* response.
#[allow(non_snake_case)]
pub fn ErrorMethodNotAllowed<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::METHOD_NOT_ALLOWED).into()
}

/// Helper function that creates wrapper of any error and generate *NOT
/// ACCEPTABLE* response.
#[allow(non_snake_case)]
pub fn ErrorNotAcceptable<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::NOT_ACCEPTABLE).into()
}

/// Helper function that creates wrapper of any error and generate *PROXY
/// AUTHENTICATION REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorProxyAuthenticationRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::PROXY_AUTHENTICATION_REQUIRED).into()
}

/// Helper function that creates wrapper of any error and generate *REQUEST
/// TIMEOUT* response.
#[allow(non_snake_case)]
pub fn ErrorRequestTimeout<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::REQUEST_TIMEOUT).into()
}

/// Helper function that creates wrapper of any error and generate *CONFLICT*
/// response.
#[allow(non_snake_case)]
pub fn ErrorConflict<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::CONFLICT).into()
}

/// Helper function that creates wrapper of any error and generate *GONE*
/// response.
#[allow(non_snake_case)]
pub fn ErrorGone<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::GONE).into()
}

/// Helper function that creates wrapper of any error and generate *LENGTH
/// REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorLengthRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::LENGTH_REQUIRED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *PAYLOAD TOO LARGE* response.
#[allow(non_snake_case)]
pub fn ErrorPayloadTooLarge<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::PAYLOAD_TOO_LARGE).into()
}

/// Helper function that creates wrapper of any error and generate
/// *URI TOO LONG* response.
#[allow(non_snake_case)]
pub fn ErrorUriTooLong<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::URI_TOO_LONG).into()
}

/// Helper function that creates wrapper of any error and generate
/// *UNSUPPORTED MEDIA TYPE* response.
#[allow(non_snake_case)]
pub fn ErrorUnsupportedMediaType<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::UNSUPPORTED_MEDIA_TYPE).into()
}

/// Helper function that creates wrapper of any error and generate
/// *RANGE NOT SATISFIABLE* response.
#[allow(non_snake_case)]
pub fn ErrorRangeNotSatisfiable<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::RANGE_NOT_SATISFIABLE).into()
}

/// Helper function that creates wrapper of any error and generate
/// *IM A TEAPOT* response.
#[allow(non_snake_case)]
pub fn ErrorImATeapot<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::IM_A_TEAPOT).into()
}

/// Helper function that creates wrapper of any error and generate
/// *MISDIRECTED REQUEST* response.
#[allow(non_snake_case)]
pub fn ErrorMisdirectedRequest<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::MISDIRECTED_REQUEST).into()
}

/// Helper function that creates wrapper of any error and generate
/// *UNPROCESSABLE ENTITY* response.
#[allow(non_snake_case)]
pub fn ErrorUnprocessableEntity<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::UNPROCESSABLE_ENTITY).into()
}

/// Helper function that creates wrapper of any error and generate
/// *LOCKED* response.
#[allow(non_snake_case)]
pub fn ErrorLocked<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::LOCKED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *FAILED DEPENDENCY* response.
#[allow(non_snake_case)]
pub fn ErrorFailedDependency<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::FAILED_DEPENDENCY).into()
}

/// Helper function that creates wrapper of any error and generate
/// *UPGRADE REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorUpgradeRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::UPGRADE_REQUIRED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *PRECONDITION FAILED* response.
#[allow(non_snake_case)]
pub fn ErrorPreconditionFailed<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::PRECONDITION_FAILED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *PRECONDITION REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorPreconditionRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::PRECONDITION_REQUIRED).into()
}

/// Helper function that creates wrapper of any error and generate
/// *TOO MANY REQUESTS* response.
#[allow(non_snake_case)]
pub fn ErrorTooManyRequests<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::TOO_MANY_REQUESTS).into()
}

/// Helper function that creates wrapper of any error and generate
/// *REQUEST HEADER FIELDS TOO LARGE* response.
#[allow(non_snake_case)]
pub fn ErrorRequestHeaderFieldsTooLarge<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE).into()
}

/// Helper function that creates wrapper of any error and generate
/// *UNAVAILABLE FOR LEGAL REASONS* response.
#[allow(non_snake_case)]
pub fn ErrorUnavailableForLegalReasons<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS).into()
}

/// Helper function that creates wrapper of any error and generate
/// *EXPECTATION FAILED* response.
#[allow(non_snake_case)]
pub fn ErrorExpectationFailed<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::EXPECTATION_FAILED).into()
}

/// Helper function that creates wrapper of any error and
/// generate *INTERNAL SERVER ERROR* response.
#[allow(non_snake_case)]
pub fn ErrorInternalServerError<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::INTERNAL_SERVER_ERROR).into()
}

/// Helper function that creates wrapper of any error and
/// generate *NOT IMPLEMENTED* response.
#[allow(non_snake_case)]
pub fn ErrorNotImplemented<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::NOT_IMPLEMENTED).into()
}

/// Helper function that creates wrapper of any error and
/// generate *BAD GATEWAY* response.
#[allow(non_snake_case)]
pub fn ErrorBadGateway<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::BAD_GATEWAY).into()
}

/// Helper function that creates wrapper of any error and
/// generate *SERVICE UNAVAILABLE* response.
#[allow(non_snake_case)]
pub fn ErrorServiceUnavailable<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::SERVICE_UNAVAILABLE).into()
}

/// Helper function that creates wrapper of any error and
/// generate *GATEWAY TIMEOUT* response.
#[allow(non_snake_case)]
pub fn ErrorGatewayTimeout<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::GATEWAY_TIMEOUT).into()
}

/// Helper function that creates wrapper of any error and
/// generate *HTTP VERSION NOT SUPPORTED* response.
#[allow(non_snake_case)]
pub fn ErrorHttpVersionNotSupported<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::HTTP_VERSION_NOT_SUPPORTED).into()
}

/// Helper function that creates wrapper of any error and
/// generate *VARIANT ALSO NEGOTIATES* response.
#[allow(non_snake_case)]
pub fn ErrorVariantAlsoNegotiates<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::VARIANT_ALSO_NEGOTIATES).into()
}

/// Helper function that creates wrapper of any error and
/// generate *INSUFFICIENT STORAGE* response.
#[allow(non_snake_case)]
pub fn ErrorInsufficientStorage<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::INSUFFICIENT_STORAGE).into()
}

/// Helper function that creates wrapper of any error and
/// generate *LOOP DETECTED* response.
#[allow(non_snake_case)]
pub fn ErrorLoopDetected<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::LOOP_DETECTED).into()
}

/// Helper function that creates wrapper of any error and
/// generate *NOT EXTENDED* response.
#[allow(non_snake_case)]
pub fn ErrorNotExtended<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::NOT_EXTENDED).into()
}

/// Helper function that creates wrapper of any error and
/// generate *NETWORK AUTHENTICATION REQUIRED* response.
#[allow(non_snake_case)]
pub fn ErrorNetworkAuthenticationRequired<T>(err: T) -> Error
where
    T: fmt::Debug + fmt::Display + 'static,
{
    InternalError::new(err, StatusCode::NETWORK_AUTHENTICATION_REQUIRED).into()
}

#[cfg(feature = "fail")]
mod failure_integration {
    use super::*;

    /// Compatibility for `failure::Error`
    impl ResponseError for failure::Error {
        fn error_response(&self) -> Response {
            Response::new(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cookie::ParseError as CookieParseError;
    use http::{Error as HttpError, StatusCode};
    use httparse;
    use std::error::Error as StdError;
    use std::io;

    #[test]
    fn test_into_response() {
        let resp: Response = ParseError::Incomplete.error_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let resp: Response = CookieParseError::EmptyName.error_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let err: HttpError = StatusCode::from_u16(10000).err().unwrap().into();
        let resp: Response = err.error_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_as_response() {
        let orig = io::Error::new(io::ErrorKind::Other, "other");
        let e: Error = ParseError::Io(orig).into();
        assert_eq!(format!("{}", e.as_response_error()), "IO error: other");
    }

    #[test]
    fn test_backtrace() {
        let e = ErrorBadRequest("err");
        let _ = e.backtrace();
    }

    #[test]
    fn test_error_cause() {
        let orig = io::Error::new(io::ErrorKind::Other, "other");
        let desc = orig.description().to_owned();
        let e = Error::from(orig);
        assert_eq!(format!("{}", e.as_response_error()), desc);
    }

    #[test]
    fn test_error_display() {
        let orig = io::Error::new(io::ErrorKind::Other, "other");
        let desc = orig.description().to_owned();
        let e = Error::from(orig);
        assert_eq!(format!("{}", e), desc);
    }

    #[test]
    fn test_error_http_response() {
        let orig = io::Error::new(io::ErrorKind::Other, "other");
        let e = Error::from(orig);
        let resp: Response = e.into();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    macro_rules! from {
        ($from:expr => $error:pat) => {
            match ParseError::from($from) {
                e @ $error => {
                    assert!(format!("{}", e).len() >= 5);
                }
                e => unreachable!("{:?}", e),
            }
        };
    }

    macro_rules! from_and_cause {
        ($from:expr => $error:pat) => {
            match ParseError::from($from) {
                e @ $error => {
                    let desc = format!("{}", e);
                    assert_eq!(desc, format!("IO error: {}", $from.description()));
                }
                _ => unreachable!("{:?}", $from),
            }
        };
    }

    #[test]
    fn test_from() {
        from_and_cause!(io::Error::new(io::ErrorKind::Other, "other") => ParseError::Io(..));
        from!(httparse::Error::HeaderName => ParseError::Header);
        from!(httparse::Error::HeaderName => ParseError::Header);
        from!(httparse::Error::HeaderValue => ParseError::Header);
        from!(httparse::Error::NewLine => ParseError::Header);
        from!(httparse::Error::Status => ParseError::Status);
        from!(httparse::Error::Token => ParseError::Header);
        from!(httparse::Error::TooManyHeaders => ParseError::TooLarge);
        from!(httparse::Error::Version => ParseError::Version);
    }

    // #[test]
    // fn failure_error() {
    //     const NAME: &str = "RUST_BACKTRACE";
    //     let old_tb = env::var(NAME);
    //     env::set_var(NAME, "0");
    //     let error = failure::err_msg("Hello!");
    //     let resp: Error = error.into();
    //     assert_eq!(
    //         format!("{:?}", resp),
    //         "Compat { error: ErrorMessage { msg: \"Hello!\" } }\n\n"
    //     );
    //     match old_tb {
    //         Ok(x) => env::set_var(NAME, x),
    //         _ => env::remove_var(NAME),
    //     }
    // }

    #[test]
    fn test_internal_error() {
        let err =
            InternalError::from_response(ParseError::Method, Response::Ok().into());
        let resp: Response = err.error_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // #[test]
    // fn test_error_downcasting_direct() {
    //     #[derive(Debug, Display)]
    //     #[display(fmt = "demo error")]
    //     struct DemoError;

    //     impl ResponseError for DemoError {}

    //     let err: Error = DemoError.into();
    //     let err_ref: &DemoError = err.downcast_ref().unwrap();
    //     assert_eq!(err_ref.to_string(), "demo error");
    // }

    // #[test]
    // fn test_error_downcasting_compat() {
    //     #[derive(Debug, Display)]
    //     #[display(fmt = "demo error")]
    //     struct DemoError;

    //     impl ResponseError for DemoError {}

    //     let err: Error = failure::Error::from(DemoError).into();
    //     let err_ref: &DemoError = err.downcast_ref().unwrap();
    //     assert_eq!(err_ref.to_string(), "demo error");
    // }

    #[test]
    fn test_error_helpers() {
        let r: Response = ErrorBadRequest("err").into();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);

        let r: Response = ErrorUnauthorized("err").into();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        let r: Response = ErrorPaymentRequired("err").into();
        assert_eq!(r.status(), StatusCode::PAYMENT_REQUIRED);

        let r: Response = ErrorForbidden("err").into();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);

        let r: Response = ErrorNotFound("err").into();
        assert_eq!(r.status(), StatusCode::NOT_FOUND);

        let r: Response = ErrorMethodNotAllowed("err").into();
        assert_eq!(r.status(), StatusCode::METHOD_NOT_ALLOWED);

        let r: Response = ErrorNotAcceptable("err").into();
        assert_eq!(r.status(), StatusCode::NOT_ACCEPTABLE);

        let r: Response = ErrorProxyAuthenticationRequired("err").into();
        assert_eq!(r.status(), StatusCode::PROXY_AUTHENTICATION_REQUIRED);

        let r: Response = ErrorRequestTimeout("err").into();
        assert_eq!(r.status(), StatusCode::REQUEST_TIMEOUT);

        let r: Response = ErrorConflict("err").into();
        assert_eq!(r.status(), StatusCode::CONFLICT);

        let r: Response = ErrorGone("err").into();
        assert_eq!(r.status(), StatusCode::GONE);

        let r: Response = ErrorLengthRequired("err").into();
        assert_eq!(r.status(), StatusCode::LENGTH_REQUIRED);

        let r: Response = ErrorPreconditionFailed("err").into();
        assert_eq!(r.status(), StatusCode::PRECONDITION_FAILED);

        let r: Response = ErrorPayloadTooLarge("err").into();
        assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let r: Response = ErrorUriTooLong("err").into();
        assert_eq!(r.status(), StatusCode::URI_TOO_LONG);

        let r: Response = ErrorUnsupportedMediaType("err").into();
        assert_eq!(r.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let r: Response = ErrorRangeNotSatisfiable("err").into();
        assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);

        let r: Response = ErrorExpectationFailed("err").into();
        assert_eq!(r.status(), StatusCode::EXPECTATION_FAILED);

        let r: Response = ErrorImATeapot("err").into();
        assert_eq!(r.status(), StatusCode::IM_A_TEAPOT);

        let r: Response = ErrorMisdirectedRequest("err").into();
        assert_eq!(r.status(), StatusCode::MISDIRECTED_REQUEST);

        let r: Response = ErrorUnprocessableEntity("err").into();
        assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let r: Response = ErrorLocked("err").into();
        assert_eq!(r.status(), StatusCode::LOCKED);

        let r: Response = ErrorFailedDependency("err").into();
        assert_eq!(r.status(), StatusCode::FAILED_DEPENDENCY);

        let r: Response = ErrorUpgradeRequired("err").into();
        assert_eq!(r.status(), StatusCode::UPGRADE_REQUIRED);

        let r: Response = ErrorPreconditionRequired("err").into();
        assert_eq!(r.status(), StatusCode::PRECONDITION_REQUIRED);

        let r: Response = ErrorTooManyRequests("err").into();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);

        let r: Response = ErrorRequestHeaderFieldsTooLarge("err").into();
        assert_eq!(r.status(), StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);

        let r: Response = ErrorUnavailableForLegalReasons("err").into();
        assert_eq!(r.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);

        let r: Response = ErrorInternalServerError("err").into();
        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let r: Response = ErrorNotImplemented("err").into();
        assert_eq!(r.status(), StatusCode::NOT_IMPLEMENTED);

        let r: Response = ErrorBadGateway("err").into();
        assert_eq!(r.status(), StatusCode::BAD_GATEWAY);

        let r: Response = ErrorServiceUnavailable("err").into();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);

        let r: Response = ErrorGatewayTimeout("err").into();
        assert_eq!(r.status(), StatusCode::GATEWAY_TIMEOUT);

        let r: Response = ErrorHttpVersionNotSupported("err").into();
        assert_eq!(r.status(), StatusCode::HTTP_VERSION_NOT_SUPPORTED);

        let r: Response = ErrorVariantAlsoNegotiates("err").into();
        assert_eq!(r.status(), StatusCode::VARIANT_ALSO_NEGOTIATES);

        let r: Response = ErrorInsufficientStorage("err").into();
        assert_eq!(r.status(), StatusCode::INSUFFICIENT_STORAGE);

        let r: Response = ErrorLoopDetected("err").into();
        assert_eq!(r.status(), StatusCode::LOOP_DETECTED);

        let r: Response = ErrorNotExtended("err").into();
        assert_eq!(r.status(), StatusCode::NOT_EXTENDED);

        let r: Response = ErrorNetworkAuthenticationRequired("err").into();
        assert_eq!(r.status(), StatusCode::NETWORK_AUTHENTICATION_REQUIRED);
    }
}
