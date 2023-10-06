use crate::difference::UpdateCommand;
use crate::style::{Color, Style};
use crate::write::{AnyWrite, Content, StrLike, WriteResult};
use crate::{coerce_fmt_write, write_any_fmt, write_any_str};
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum OSControl<'a, S: 'a + ToOwned + ?Sized>
where
    S: fmt::Debug,
{
    Title,
    Link { url: Content<'a, S> },
}

impl<'a, S: 'a + ToOwned + ?Sized> Clone for OSControl<'a, S>
where
    S: fmt::Debug,
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
pub struct AnsiGenericString<'a, S: 'a + ToOwned + ?Sized>
where
    S: fmt::Debug,
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
impl<'a, S: 'a + ToOwned + ?Sized> Clone for AnsiGenericString<'a, S>
where
    S: fmt::Debug,
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

impl<'a, S: 'a + ToOwned + ?Sized> From<&'a S> for AnsiGenericString<'a, S>
where
    S: fmt::Debug,
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

impl<'a, S: 'a + ToOwned + ?Sized> From<fmt::Arguments<'a>> for AnsiGenericString<'a, S>
where
    S: fmt::Debug,
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
    S: fmt::Debug,
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

    pub fn oscontrol(&self) -> &Option<OSControl<'a, S>> {
        &self.oscontrol
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
pub struct AnsiGenericStrings<'a, S: 'a + ToOwned + ?Sized>
where
    S: fmt::Debug,
{
    contents: Vec<Content<'a, S>>,
    style_updates: Vec<StyleUpdate>,
    oscontrols: Vec<Option<OSControl<'a, S>>>,
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericStrings<'a, S>
where
    S: fmt::Debug,
{
    pub fn empty(capacity: usize) -> Self {
        Self {
            contents: Vec::with_capacity(capacity),
            style_updates: Vec::with_capacity(capacity),
            oscontrols: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, s: AnsiGenericString<'a, S>) {
        let index = self.push_content(s.content().clone());
        self.push_style(*s.style(), index);
        self.push_oscontrol(s.oscontrol().clone());
    }

    fn push_style(&mut self, next: Style, begins_at: usize) {
        let instructions = self
            .style_updates
            .last()
            .map(|style_update| style_update.command.update_relative(next))
            .unwrap_or_else(|| {
                if next.is_plain() {
                    UpdateCommand::DoNothing
                } else {
                    UpdateCommand::Prefix(next)
                }
            });

        self.style_updates.push(StyleUpdate {
            begins_at,
            command: instructions,
        })
    }

    #[inline]
    fn push_oscontrol(&mut self, oscontrol: Option<OSControl<'a, S>>) {
        self.oscontrols.push(oscontrol)
    }

    #[inline]
    fn push_content(&mut self, content: Content<'a, S>) -> usize {
        self.contents.push(content);
        self.contents.len()
    }

    fn write_iter(&self) -> WriteIter<'_, '_, S> {
        WriteIter {
            style_iter: StyleIter {
                cursor: 0,
                instructions: &self.style_updates,
                next_update: None,
                current: None,
            },
            content_iter: ContentIter {
                cursor: 0,
                contents: &self.contents,
                oscontrols: &self.oscontrols,
            },
        }
    }
}

pub struct StyleIter<'a> {
    cursor: usize,
    instructions: &'a Vec<StyleUpdate>,
    next_update: Option<StyleUpdate>,
    current: Option<StyleUpdate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleUpdate {
    command: UpdateCommand,
    begins_at: usize,
}

impl<'a> StyleIter<'a> {
    fn get_next_update(&mut self) {
        self.cursor += 1;
        self.next_update = self.instructions.get(self.cursor).copied();
    }
}

impl<'b> Iterator for StyleIter<'b> {
    type Item = UpdateCommand;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.current, self.next_update) {
            (None, None) => {
                self.current = self.instructions.get(self.cursor).copied();
                self.get_next_update();
                self.current
            }
            (current, Some(next_update)) => {
                if self.cursor < next_update.begins_at {
                    current
                } else {
                    self.current = self.next_update.take();
                    self.cursor += 1;
                    self.next_update = self.instructions.get(self.cursor).copied();
                    self.current
                }
            }
            (Some(current), None) => current.into(),
        }
        .map(|u| u.command)
    }
}

pub struct ContentIter<'b, 'a: 'b, S: 'a + ToOwned + ?Sized>
where
    S: fmt::Debug,
{
    cursor: usize,
    contents: &'b Vec<Content<'a, S>>,
    oscontrols: &'b Vec<Option<OSControl<'a, S>>>,
}

impl<'b, 'a: 'b, S: 'a + ToOwned + ?Sized> Iterator for ContentIter<'b, 'a, S>
where
    S: fmt::Debug,
{
    type Item = (Content<'a, S>, Option<OSControl<'a, S>>);

    fn next(&mut self) -> Option<Self::Item> {
        let r = self.contents.get(self.cursor).map(|content| {
            (
                content.clone(),
                self.oscontrols.get(self.cursor).cloned().flatten(),
            )
        });

        if r.is_some() {
            self.cursor += 1;
        }
        r
    }
}

pub struct WriteIter<'b, 'a, S: 'a + ToOwned + ?Sized>
where
    S: fmt::Debug,
{
    style_iter: StyleIter<'a>,
    content_iter: ContentIter<'b, 'a, S>,
}

impl<'b, 'a, S: 'a + ToOwned + ?Sized> Iterator for WriteIter<'b, 'a, S>
where
    S: fmt::Debug,
{
    type Item = (UpdateCommand, Content<'a, S>, Option<OSControl<'a, S>>);

    fn next(&mut self) -> Option<Self::Item> {
        let (content, oscontrol) = self.content_iter.next()?;
        let update_command = self.style_iter.next().unwrap_or_default();
        Some((update_command, content, oscontrol))
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> FromIterator<&'a AnsiGenericString<'a, S>>
    for AnsiGenericStrings<'a, S>
where
    S: fmt::Debug,
{
    fn from_iter<Iterable: IntoIterator<Item = &'a AnsiGenericString<'a, S>>>(
        iter: Iterable,
    ) -> Self {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let count = upper.unwrap_or(lower);
        let mut ansi_strings = AnsiGenericStrings::empty(count);
        for s in iter {
            ansi_strings.push(s.clone());
        }
        ansi_strings
    }
}

/// A set of `AnsiString`s collected together, in order to be written with a
/// minimum of control characters.
pub type AnsiStrings<'a> = AnsiGenericStrings<'a, str>;

/// A function to construct an `AnsiStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiStrings<'a>(arg: &'a [AnsiString<'a>]) -> AnsiStrings<'a> {
    AnsiGenericStrings::from_iter(arg)
}

/// A set of `AnsiByteString`s collected together, in order to be
/// written with a minimum of control characters.
pub type AnsiByteStrings<'a> = AnsiGenericStrings<'a, [u8]>;

/// A function to construct an `AnsiByteStrings` instance.
#[allow(non_snake_case)]
pub fn AnsiByteStrings<'a>(arg: &'a [AnsiByteString<'a>]) -> AnsiByteStrings<'a> {
    AnsiGenericStrings::from_iter(arg)
}

// ---- paint functions ----

impl Style {
    /// Paints the given text with this color, returning an ANSI string.
    #[must_use]
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
        S: fmt::Debug,
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
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Content<'a, S>>,
        S: fmt::Debug,
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
        self.write_to_any(coerce_fmt_write!(f))
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
    S: fmt::Debug,
{
    // write the part within the styling prefix and suffix
    fn write_inner<T: 'a + ToOwned + ?Sized, W: AnyWrite<Buf = T> + ?Sized>(
        content: &Content<'a, S>,
        oscontrol: &Option<OSControl<'a, S>>,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, T> + AsRef<T>,
        str: AsRef<T>,
    {
        match oscontrol {
            Some(OSControl::Link { url: u, .. }) => {
                write_any_str!(w, "\x1B]8;;")?;
                u.write_to(w)?;
                write_any_str!(w, "\x1B\x5C")?;
                content.write_to(w)?;
                write_any_str!(w, "\x1B]8;;\x1B\x5C")
            }
            Some(OSControl::Title) => {
                write_any_str!(w, "\x1B]2;")?;
                content.write_to(w)?;
                write_any_str!(w, "\x1B\x5C")
            }
            None => content.write_to(w),
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
        Self::write_inner(&self.content, &self.oscontrol, w)?;
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
    S: fmt::Debug,
{
    fn write_to_any<T: 'a + ToOwned + ?Sized, W: AnyWrite<Buf = T> + ?Sized>(
        &'a self,
        w: &mut W,
    ) -> WriteResult<W::Error>
    where
        S: StrLike<'a, T> + AsRef<T>,
        str: AsRef<T>,
    {
        let mut last_is_plain = true;

        for (style_command, content, oscontrol) in self.write_iter() {
            match style_command {
                UpdateCommand::Prefix(style) => {
                    style.write_prefix(w)?;
                    last_is_plain = style.is_plain();
                }
                UpdateCommand::DoNothing => {}
            }
            AnsiGenericString::write_inner(&content, &oscontrol, w)?;
        }

        if last_is_plain {
            Ok(())
        } else {
            Style::default().prefix_with_reset().write_prefix(w)
        }
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
        let joined = AnsiStrings(&[before_g.clone(), unstyled.clone()]).to_string();
        assert!(
            joined.starts_with("\x1B[32mBefore is Green. \x1B[0m"),
            "{:?} does not start with {:?}",
            joined,
            "\x1B[32mBefore is Green. \x1B[0m"
        );
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
        dbg!("link: {:?}", &link);
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
