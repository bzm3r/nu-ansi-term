use std::borrow::Cow;
use std::fmt;
use std::fmt::Debug;
use std::io;

/// Helper to alias  over [`fmt::Result`], or [`io::Result`] depending on the
/// error type used ([`fmt::Error`] or [`io::Error`]).
pub type WriteResult<E> = Result<(), E>;

/// Takes an [`AnyWrite`] implementor and arguments necessary to build
/// [`fmt::Arguments`].
/// ```
/// use std::fmt;
/// use nu_ansi_term::{Color, fmt_write, write_any_fmt, AnyWrite};
///
/// let mut s = String::new();
/// let mut t = String::new();
/// fmt_write!(&mut s).write_any_fmt(format_args!("{}", Color::Red.paint("hello world!")));
/// write_any_fmt!(fmt_write!(&mut t), "{}", Color::Red.paint("hello world!"));
/// assert_eq!(s, t);
/// ```
#[macro_export]
macro_rules! write_any_fmt {
    ($w:expr, $($args:tt)*) => {
        $w.write_any_fmt(std::format_args!($($args)*))
    };
}

/// Takes an [`AnyWrite`] implementor and writes some [`StrLike`] content to it.
/// ```
/// use std::fmt;
/// use nu_ansi_term::{fmt_write, write_any_str, AnyWrite, StrLike};
///
/// let mut s = String::new();
/// let mut t = String::new();
/// fmt_write!(&mut s).write_any_str("hello world!");
/// write_any_str!(fmt_write!(&mut t), "hello world!");
/// assert_eq!(s, t);
/// ```
#[macro_export]
macro_rules! write_any_str {
    ($w:expr, $($args:tt)*) => {
        $($args)*.write_str_to($w)
    };
}

/// Coerce the given writer into `&mut dyn fmt::Write`. It is a compile-time
/// error if this is not possible.
#[macro_export]
macro_rules! fmt_write {
    ($w:expr) => {{
        let w: &mut dyn fmt::Write = $w;
        w
    }};
}

/// Coerce the given writer into `&mut dyn io::Write`. It is a compile-time
/// error if this is not possible.
#[macro_export]
macro_rules! io_write {
    ($w:expr) => {{
        let w: &mut dyn io::Write = $w;
        w
    }};
}

/// Allows for generalization over [`fmt::Write`] and [`io::Write`] implementors.
pub trait AnyWrite {
    /// Type of string-like data buffers accepted by this writer ([`str`] for
    /// [`fmt::Write`] and [`[u8]`] [`io::Write`]).
    type Buf: ?Sized + ToOwned;
    /// Type of error produced by this writer ([`fmt::Error`] for
    /// [`fmt::Write`] and [`io::Error`] [`io::Write`]).
    type Error;

    /// Write [`fmt::Arguments`] data (created using [`format_args!`] macro) to this writer.
    fn write_any_fmt(&mut self, args: fmt::Arguments) -> WriteResult<Self::Error>;

    /// Write [`AnyWrite::Buf`] type data to this writer.
    fn write_any_str(&mut self, s: &Self::Buf) -> WriteResult<Self::Error>;
}

impl<'a> AnyWrite for dyn fmt::Write + 'a {
    type Buf = str;
    type Error = fmt::Error;

    fn write_any_fmt(&mut self, args: fmt::Arguments) -> WriteResult<Self::Error> {
        fmt::Write::write_fmt(self, args)
    }

    fn write_any_str(&mut self, s: &Self::Buf) -> WriteResult<Self::Error> {
        fmt::Write::write_str(self, s)
    }
}

impl<'a> AnyWrite for dyn io::Write + 'a {
    type Buf = [u8];
    type Error = io::Error;

    fn write_any_fmt(&mut self, args: fmt::Arguments) -> WriteResult<Self::Error> {
        io::Write::write_fmt(self, args)
    }

    fn write_any_str(&mut self, s: &Self::Buf) -> WriteResult<Self::Error> {
        io::Write::write_all(self, s)
    }
}

/// Implementors can be converted into a reference to string-like data buffer
/// accepted by `W`.
pub trait StrLike<'a, W: AnyWrite + ?Sized>
where
    Self: AsRef<W::Buf>,
{
    /// Write string-like data to the writer.
    fn write_str_to(&self, w: &mut W) -> WriteResult<W::Error>;
}

impl<'a, W: AnyWrite + ?Sized, S: ?Sized + ToOwned + AsRef<W::Buf>> StrLike<'a, W> for S {
    fn write_str_to(&self, w: &mut W) -> WriteResult<W::Error> {
        w.write_any_str(self.as_ref())
    }
}

/// Enumerates the two types of content an [`AnyWrite`] implementor can handle:
/// * [fmt::Arguments]
/// * anything that implements [`AsRef<AnyWrite::Buf>`] (conveniently
///   stored in either reference or owned format within a [`Cow`]).
pub enum Content<'a, S: ?Sized + ToOwned> {
    /// Content is [`fmt::Arguments`].
    FmtArgs(fmt::Arguments<'a>),
    /// Content is a reference to something that implements [`ToOwned`], or the
    /// [`ToOwned::Owned`] variant specified by that implementation.
    StrLike(Cow<'a, S>),
}

impl<'a, S: ?Sized + ToOwned> ToString for Content<'a, S>
where
    S: AsRef<str>,
{
    fn to_string(&self) -> String {
        match self {
            Content::FmtArgs(x) => format!("{}", x),
            Content::StrLike(x) => {
                let mut s = String::new();
                <S as StrLike<'a, dyn fmt::Write>>::write_str_to(x, fmt_write!(&mut s)).unwrap();
                s
            }
        }
    }
}

impl<'a, S: ?Sized + ToOwned> Clone for Content<'a, S> {
    fn clone(&self) -> Self {
        match self {
            Self::FmtArgs(x) => Self::FmtArgs(*x),
            Self::StrLike(x) => Self::StrLike(x.clone()),
        }
    }
}

impl<'a, S: ?Sized + ToOwned> Debug for Content<'a, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FmtArgs(x) => f.debug_tuple("FmtArgs").field(x).finish(),
            Self::StrLike(x) => f.debug_tuple("StrLike").field(&x.as_ref()).finish(),
        }
    }
}

impl<'a, S: ?Sized + ToOwned> Content<'a, S> {
    /// Write content to the given writer.
    pub fn write_to<T: ?Sized + ToOwned, W: AnyWrite<Buf = T> + ?Sized>(
        &self,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, W>,
    {
        match self {
            Content::FmtArgs(args) => w.write_any_fmt(*args),
            Content::StrLike(s) => <S as StrLike<'a, W>>::write_str_to(s, w),
        }
    }
}

impl<'a, S: ?Sized + ToOwned, T: ?Sized + ToOwned> From<&'a T> for Content<'a, S>
where
    S: Debug,
    T: AsRef<S>,
{
    fn from(s: &'a T) -> Self {
        Content::StrLike(Cow::Borrowed(s.as_ref()))
    }
}

impl<'a, S: ?Sized + ToOwned> From<fmt::Arguments<'a>> for Content<'a, S>
where
    S: Debug,
{
    fn from(args: fmt::Arguments<'a>) -> Self {
        Content::FmtArgs(args)
    }
}

impl<'a> From<String> for Content<'a, str> {
    fn from(s: String) -> Self {
        Content::StrLike(Cow::Owned(s))
    }
}
