use crate::style::{Color, Style};
use crate::write::{AnyWrite, Content, StrLike, WriteResult};
use crate::{write_any_fmt, write_any_str};
use std::fmt;
use std::io;

#[derive(Debug)]
enum OSControl<'a, S: 'a + ToOwned + ?Sized>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    Title,
    Link { url: Content<'a, S> },
}

impl<'a, S: ToOwned + ?Sized> Clone for OSControl<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn clone(&self) -> Self {
        match self {
            Self::Link { url: u } => Self::Link { url: u.clone() },
            Self::Title => Self::Title,
        }
    }
}

/// An `AnsiGenericString` includes a generic string type and a `Style` to
/// display that string.  `AnsiString` and `AnsiByteString` are aliases for
/// this type on `str` and `\[u8]`, respectively.
#[derive(Debug)]
pub struct AnsiGenericString<'a, S: ToOwned + ?Sized>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    pub(crate) style: Style,
    pub(crate) content: Content<'a, S>,
    oscontrol: Option<OSControl<'a, S>>,
}

/// Cloning an `AnsiGenericString` will clone its underlying string.
///
/// # Examples
///
/// ```
/// use nu_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// let clone_string = plain_string.clone();
/// assert_eq!(clone_string, plain_string);
/// ```
impl<'a, S: ToOwned + ?Sized> Clone for AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn clone(&self) -> AnsiGenericString<'a, S> {
        AnsiGenericString {
            style: self.style,
            content: self.content.clone(),
            oscontrol: self.oscontrol.clone(),
        }
    }
}

// You might think that the hand-written Clone impl above is the same as the
// one that gets generated with #[derive]. But it’s not *quite* the same!
//
// `str` is not Clone, and the derived Clone implementation puts a Clone
// constraint on the S type parameter (generated using --pretty=expanded):
//
//                  ↓_________________↓
//     impl <'a, S: ::std::clone::Clone + 'a + ToOwned + ?Sized> ::std::clone::Clone
//     for ANSIGenericString<'a, S> where
//     <S as ToOwned>::Owned: fmt::Debug { ... }
//
// This resulted in compile errors when you tried to derive Clone on a type
// that used it:
//
//     #[derive(PartialEq, Debug, Clone, Default)]
//     pub struct TextCellContents(Vec<AnsiString<'static>>);
//                                 ^^^^^^^^^^^^^^^^^^^^^^^^^
//     error[E0277]: the trait `std::clone::Clone` is not implemented for `str`
//
// The hand-written impl above can ignore that constraint and still compile.

impl<'a, S: ToOwned + ?Sized> From<&'a S> for AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
    S: AsRef<S>,
{
    fn from(s: &'a S) -> Self {
        AnsiGenericString {
            style: Style::default(),
            content: s.into(),
            oscontrol: None,
        }
    }
}

impl<'a, S: ToOwned + ?Sized> From<fmt::Arguments<'a>> for AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn from(args: fmt::Arguments<'a>) -> Self {
        AnsiGenericString {
            style: Style::default(),
            content: args.into(),
            oscontrol: None,
        }
    }
}

/// An ANSI String is a string coupled with the `Style` to display it
/// in a terminal.
///
/// Although not technically a string itself, it can be turned into
/// one with the `to_string` method.
///
/// # Examples
///
/// ```
/// use nu_ansi_term::AnsiString;
/// use nu_ansi_term::Color::Red;
///
/// let red_string = Red.paint("a red string");
/// println!("{}", red_string);
/// ```
///
/// ```
/// use nu_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// ```
pub type AnsiString<'a> = AnsiGenericString<'a, str>;

/// An `AnsiByteString` represents a formatted series of bytes.  Use
/// `AnsiByteString` when styling text with an unknown encoding.
pub type AnsiByteString<'a> = AnsiGenericString<'a, [u8]>;

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    /// Directly access the style
    pub const fn style(&self) -> &Style {
        &self.style
    }

    /// Directly access the style mutably
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    pub fn content(&self) -> &Content<'a, S> {
        &self.content
    }

    // Instances that imply wrapping in OSC sequences
    // and do not get displayed in the terminal text
    // area.
    //
    /// Produce an ANSI string that changes the title shown
    /// by the terminal emulator.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::AnsiGenericString;
    /// let title_string = AnsiGenericString::title("My Title");
    /// println!("{}", title_string);
    /// ```
    /// Should produce an empty line but set the terminal title.
    pub fn title<I>(s: I) -> Self
    where
        I: Into<Content<'a, S>>,
    {
        Self {
            style: Style::default(),
            content: s.into(),
            oscontrol: Some(OSControl::<S>::Title),
        }
    }

    //
    // Annotations (OSC sequences that do more than wrap)
    //

    /// Cause the styled ANSI string to link to the given URL
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color::Red;
    ///
    /// let link_string = Red.paint("a red string").hyperlink("https://www.example.com");
    /// println!("{}", link_string);
    /// ```
    /// Should show a red-painted string which, on terminals
    /// that support it, is a clickable hyperlink.
    pub fn hyperlink<I>(mut self, url: I) -> Self
    where
        I: Into<Content<'a, S>>,
    {
        self.oscontrol = Some(OSControl::Link { url: url.into() });
        self
    }

    /// Get any URL associated with the string
    pub fn url_string(&self) -> Option<&Content<'_, S>> {
        self.oscontrol.as_ref().and_then(|osc| {
            if let OSControl::Link { url } = osc {
                Some(url)
            } else {
                None
            }
        })
    }
}

/// A set of `AnsiGenericStrings`s collected together, in order to be
/// written with a minimum of control characters.
#[derive(Debug)]
pub struct AnsiGenericStrings<'a, S: ToOwned + ?Sized>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    _args: Vec<Content<'a, S>>,
    _styles: Vec<(usize, Style)>,
    _oscontrols: Vec<OSControl<'a, S>>,
}

/// A set of `AnsiString`s collected together, in order to be written with a
/// minimum of control characters.
pub type AnsiStrings<'a> = AnsiGenericStrings<'a, str>;

/// A function to construct an `AnsiStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiStrings<'a>(_arg: &'a [AnsiString<'a>]) -> AnsiStrings<'a> {
    unimplemented!()
}

/// A set of `AnsiByteString`s collected together, in order to be
/// written with a minimum of control characters.
pub type AnsiByteStrings<'a> = AnsiGenericStrings<'a, [u8]>;

/// A function to construct an `AnsiByteStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiByteStrings<'a>(_arg: &'a [AnsiByteString<'a>]) -> AnsiByteStrings<'a> {
    unimplemented!()
}

// ---- paint functions ----

impl Style {
    /// Paints the given text with this color, returning an ANSI string.
    #[must_use]
    pub fn paint<'a, I, S: ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug,
    {
        AnsiGenericString {
            style: self,
            content: input.into(),
            oscontrol: None,
        }
    }
}

impl Color {
    /// Paints the given text with this color, returning an ANSI string.
    /// This is a short-cut so you don’t have to use `Blue.normal()` just
    /// to get blue text.
    ///
    /// ```
    /// use nu_ansi_term::Color::Blue;
    /// println!("{}", Blue.paint("da ba dee"));
    /// ```
    #[must_use]
    pub fn paint<'a, I, S: ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug,
    {
        AnsiGenericString {
            content: input.into(),
            style: self.normal(),
            oscontrol: None,
        }
    }
}

// ---- writers for individual ANSI strings ----

impl<'a> fmt::Display for AnsiString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let w: &mut dyn fmt::Write = f;
        self.write_to_any(w)
    }
}

impl<'a> AnsiByteString<'a> {
    /// Write an `AnsiByteString` to an `io::Write`.  This writes the escape
    /// sequences for the associated `Style` around the bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        let w: &mut dyn io::Write = w;
        self.write_to_any(w)
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    // write the part within the styling prefix and suffix
    fn write_inner<T: 'a + ToOwned + ?Sized, W: AnyWrite<Buf = T> + ?Sized>(
        &self,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, T> + AsRef<T>,
        str: AsRef<T>,
    {
        match &self.oscontrol {
            Some(OSControl::Link { url: u, .. }) => {
                write_any_str!(w, "\x1B]8;;")?;
                u.write_to(w)?;
                write_any_str!(w, "\x1B\x5C")?;
                self.content.write_to(w)?;
                write_any_str!(w, "\x1B]8;;\x1B\x5C")
            }
            Some(OSControl::Title) => {
                write_any_str!(w, "\x1B]2;")?;
                self.content.write_to(w)?;
                write_any_str!(w, "\x1B\x5C")
            }
            None => self.content.write_to(w),
        }
    }

    fn write_to_any<T: 'a + ToOwned + ?Sized, W: AnyWrite<Buf = T> + ?Sized>(
        &self,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, T> + AsRef<T>,
        str: AsRef<T>,
    {
        write_any_fmt!(w, "{}", self.style.prefix())?;
        self.write_inner(w)?;
        write_any_fmt!(w, "{}", self.style.suffix())
    }
}

// ---- writers for combined ANSI strings ----

impl<'a> fmt::Display for AnsiStrings<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let f: &mut dyn fmt::Write = f;
        self.write_to_any(f)
    }
}

impl<'a> AnsiByteStrings<'a> {
    /// Write `AnsiByteStrings` to an `io::Write`.  This writes the minimal
    /// escape sequences for the associated `Style`s around each set of
    /// bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        let w: &mut dyn io::Write = w;
        self.write_to_any(w)
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericStrings<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn write_to_any<W: AnyWrite<Buf = S> + ?Sized>(&self, _w: &mut W) -> WriteResult<W::Error> {
        unimplemented!()
        // use self::Difference::*;

        // let first = match self.0.first() {
        //     None => return Ok(()),
        //     Some(f) => f,
        // };

        // write_any_fmt!(w, "{}", first.style.prefix())?;
        // first.write_inner(w)?;

        // for window in self.0.windows(2) {
        //     match Difference::between(&window[0].style, &window[1].style) {
        //         ExtraStyles(style) => write_any_fmt!(w, "{}", style.prefix())?,
        //         Reset => write_any_fmt!(w, "{}{}", RESET, window[1].style.prefix())?,
        //         Empty => { /* Do nothing! */ }
        //     }

        //     window[1].write_inner(w)?;
        // }

        // // Write the final reset string after all of the AnsiStrings have been
        // // written, *except* if the last one has no styles, because it would
        // // have already been written by this point.
        // if let Some(last) = self.0.last() {
        //     if !last.style.is_plain() {
        //         write_any_fmt!(w, "{}", RESET)?;
        //     }
        // }

        // Ok(())
    }
}

// ---- tests ----

#[cfg(test)]
mod tests {
    pub use super::super::{AnsiGenericString, AnsiStrings};
    pub use crate::style::Color::*;
    pub use crate::style::Style;

    #[test]
    fn no_control_codes_for_plain() {
        let one = Style::default().paint("one");
        let two = Style::default().paint("two");
        let output = AnsiStrings(&[one, two]).to_string();
        assert_eq!(output, "onetwo");
    }

    // NOTE: unstyled because it could have OSC escape sequences
    fn idempotent(unstyled: AnsiGenericString<'_, str>) {
        let before_g = Green.paint("Before is Green. ");
        let before = Style::default().paint("Before is Plain. ");
        let after_g = Green.paint(" After is Green.");
        let after = Style::default().paint(" After is Plain.");
        let unstyled_s = unstyled.clone().to_string();

        // check that RESET precedes unstyled
        let joined = AnsiStrings(&[before_g, unstyled.clone()]).to_string();
        assert!(joined.starts_with("\x1B[32mBefore is Green. \x1B[0m"));
        assert!(
            joined.ends_with(unstyled_s.as_str()),
            "{:?} does not end with {:?}",
            joined,
            unstyled_s
        );

        // check that RESET does not follow unstyled when appending styled
        let joined = AnsiStrings(&[unstyled.clone(), after_g.clone()]).to_string();
        assert!(
            joined.starts_with(unstyled_s.as_str()),
            "{:?} does not start with {:?}",
            joined,
            unstyled_s
        );
        assert!(joined.ends_with("\x1B[32m After is Green.\x1B[0m"));

        // does not introduce spurious SGR codes (reset or otherwise) adjacent
        // to plain strings
        let joined = AnsiStrings(&[unstyled.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[before.clone(), unstyled.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[before.clone(), unstyled.clone(), after.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[unstyled.clone(), after.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
    }

    #[test]
    fn title() {
        let title = AnsiGenericString::title("Test Title");
        assert_eq!(&title.to_string(), "\x1B]2;Test Title\x1B\\");
        idempotent(title)
    }

    #[test]
    fn hyperlink() {
        let styled = Red
            .paint("Link to example.com.")
            .hyperlink("https://example.com");
        assert_eq!(
            styled.to_string(),
            "\x1B[31m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"
        );
    }

    #[test]
    fn hyperlinks() {
        let before = Green.paint("Before link. ");
        let link = Blue
            .underline()
            .paint("Link to example.com.")
            .hyperlink("https://example.com");
        let after = Green.paint(" After link.");

        // Assemble with link by itself
        let joined = AnsiStrings(&[link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));

        // Assemble with link in the middle
        let joined = AnsiStrings(&[before.clone(), link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));

        // Assemble with link first
        let joined = AnsiStrings(&[link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m\x1B[32m After link.\x1B[0m"));

        // Assemble with link at the end
        let joined = AnsiStrings(&[before.clone(), link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x1B]8;;https://example.com\x1B\\Link to example.com.\x1B]8;;\x1B\\\x1B[0m"));
    }
}
