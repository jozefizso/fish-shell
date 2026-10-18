#[cfg(windows)]
use crate::env::Environment;
use crate::prelude::*;
use fish_color::Color;
use fish_wgetopt::{ArgType, WGetopter, WOption, wopt};

#[derive(Copy, Clone, Default)]
pub struct ColorSupport {
    pub term_256color: bool,
    pub term_24bit: bool,
}

#[cfg(windows)]
/// Interpret fish's configured color-depth policy without querying a terminal.
/// Batch ANSI output uses this policy for serialization, not as a claim about console support.
pub(crate) fn color_support_from_env(vars: &dyn Environment) -> ColorSupport {
    use fish_wcstringutil::bool_from_string;
    let is_xterm_16color = vars
        .get_unless_empty(L!("TERM"))
        .is_some_and(|term| term.as_list()[0] == "xterm-16color");
    let term_256color = vars
        .get(L!("fish_term256"))
        .map_or(!is_xterm_16color, |value| {
            bool_from_string(&value.as_string())
        });
    let term_24bit = if let Some(value) = vars.get(L!("fish_term24bit")) {
        bool_from_string(&value.as_string())
    } else if vars.get(L!("STY")).is_some() {
        false
    } else if let Some(value) = vars.get(L!("COLORTERM")) {
        let value = value.as_string();
        value == "truecolor" || value == "24bit"
    } else {
        !is_xterm_16color
            && vars
                .get_unless_empty(L!("TERM_PROGRAM"))
                .is_none_or(|value| value.as_list()[0] != "Apple_Terminal")
    };
    ColorSupport {
        term_256color,
        term_24bit,
    }
}

/// Pick a configured color using the shared RGB-versus-named fallback policy.
pub fn best_color(candidates: impl Iterator<Item = Color>, support: ColorSupport) -> Option<Color> {
    let mut first = None;
    let mut first_rgb = None;
    let mut first_named = None;
    for color in candidates {
        if first.is_none() {
            first = Some(color);
        }
        if first_rgb.is_none() && color.is_rgb() {
            first_rgb = Some(color);
        }
        if first_named.is_none() && color.is_named() {
            first_named = Some(color);
        }
    }
    (if (first_rgb.is_some() && support.term_256color) || first_named.is_none() {
        first_rgb
    } else {
        first_named
    })
    .or(first)
}

fn default_color_support() -> ColorSupport {
    #[cfg(unix)]
    {
        crate::terminal::get_color_support()
    }
    #[cfg(windows)]
    {
        color_support_from_env(crate::env::EnvStack::globals())
    }
}

pub(crate) trait StyleSet {
    fn union_prefer_right(self, other: Self) -> Self;
    fn difference_prefer_empty(self, other: Self) -> Self;
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub(crate) enum ResettableStyle<T = ()>
where
    T: Copy + std::fmt::Debug,
{
    #[default]
    Unchanged,
    Off,
    On(T),
}

impl<T> StyleSet for ResettableStyle<T>
where
    T: Copy + std::fmt::Debug,
{
    fn union_prefer_right(self, other: Self) -> Self {
        if matches!(other, Self::Unchanged) {
            self
        } else {
            other
        }
    }

    fn difference_prefer_empty(self, other: Self) -> Self {
        if !matches!(other, Self::Unchanged) {
            Self::Unchanged
        } else {
            self
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum UnderlineStyle {
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct TextStyling {
    pub(crate) bold: bool,
    pub(crate) underline_style: ResettableStyle<UnderlineStyle>,
    pub(crate) italics: ResettableStyle,
    pub(crate) dim: bool,
    pub(crate) reverse: ResettableStyle,
    pub(crate) strikethrough: ResettableStyle,
}

impl TextStyling {
    pub(crate) const fn terminal_default() -> Self {
        Self {
            bold: false,
            underline_style: ResettableStyle::Off,
            italics: ResettableStyle::Off,
            dim: false,
            reverse: ResettableStyle::Off,
            strikethrough: ResettableStyle::Off,
        }
    }
    pub(crate) const fn unknown() -> Self {
        Self {
            bold: false,
            underline_style: ResettableStyle::Unchanged,
            italics: ResettableStyle::Unchanged,
            dim: false,
            reverse: ResettableStyle::Unchanged,
            strikethrough: ResettableStyle::Unchanged,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::unknown()
    }

    #[cfg(test)]
    pub(crate) fn all_set(&self) -> bool {
        (self.underline_style != ResettableStyle::Unchanged)
            && (self.italics != ResettableStyle::Unchanged)
            && (self.reverse != ResettableStyle::Unchanged)
            && (self.strikethrough != ResettableStyle::Unchanged)
    }

    pub(crate) fn union_prefer_right(self, other: Self) -> Self {
        Self {
            bold: self.is_bold() || other.is_bold(),
            underline_style: self
                .underline_style
                .union_prefer_right(other.underline_style),
            italics: self.italics.union_prefer_right(other.italics),
            dim: self.is_dim() || other.is_dim(),
            reverse: self.reverse.union_prefer_right(other.reverse),
            strikethrough: self.strikethrough.union_prefer_right(other.strikethrough),
        }
    }
    pub(crate) fn difference_prefer_empty(self, other: Self) -> Self {
        Self {
            bold: self.is_bold() && !other.is_bold(),
            underline_style: self
                .underline_style
                .difference_prefer_empty(other.underline_style),
            italics: self.italics.difference_prefer_empty(other.italics),
            dim: self.is_dim() && !other.is_dim(),
            reverse: self.reverse.difference_prefer_empty(other.reverse),
            strikethrough: self
                .strikethrough
                .difference_prefer_empty(other.strikethrough),
        }
    }

    /// Returns whether the text face is bold.
    pub const fn is_bold(self) -> bool {
        self.bold
    }

    #[cfg(test)]
    pub const fn underline_style(self) -> ResettableStyle<UnderlineStyle> {
        self.underline_style
    }

    /// Set the given underline style.
    pub fn inject_underline(&mut self, underline: ResettableStyle<UnderlineStyle>) {
        self.underline_style = underline;
    }

    /// Returns whether the text face is dim.
    pub const fn is_dim(self) -> bool {
        self.dim
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextFace {
    pub(crate) fg: Color,
    pub(crate) bg: Color,
    pub(crate) underline_color: Color,
    pub(crate) style: TextStyling,
}

impl TextFace {
    pub const fn terminal_default() -> Self {
        Self {
            fg: Color::Normal,
            bg: Color::Normal,
            underline_color: Color::Normal,
            style: TextStyling::terminal_default(),
        }
    }
    pub const fn unknown() -> Self {
        Self {
            fg: Color::None,
            bg: Color::None,
            underline_color: Color::None,
            style: TextStyling::unknown(),
        }
    }

    pub fn new(fg: Color, bg: Color, underline_color: Color, style: TextStyling) -> Self {
        Self {
            fg,
            bg,
            underline_color,
            style,
        }
    }

    #[cfg(test)]
    pub(crate) fn all_set(&self) -> bool {
        !self.fg.is_none()
            && !self.bg.is_none()
            && !self.underline_color.is_none()
            && self.style.all_set()
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct SpecifiedTextFace {
    pub(crate) fg: Option<Color>,
    pub(crate) bg: Option<Color>,
    pub(crate) underline_color: Option<Color>,
    pub(crate) style: TextStyling,
    pub(crate) reset: bool,
}

impl Default for SpecifiedTextFace {
    fn default() -> Self {
        SpecifiedTextFace {
            fg: Default::default(),
            bg: Default::default(),
            underline_color: Default::default(),
            style: TextStyling::unknown(),
            reset: false,
        }
    }
}

pub(crate) fn parse_text_face(arguments: &[WString]) -> SpecifiedTextFace {
    parse_text_face_with_color_support(arguments, default_color_support())
}

pub(crate) fn parse_text_face_with_color_support(
    arguments: &[WString],
    support: ColorSupport,
) -> SpecifiedTextFace {
    #[cfg(unix)]
    let words = arguments.iter().map(|argument| argument.as_utfstr());
    // Native inherited environment values are scalars. A color setting's exported form
    // joins its option/color words with spaces, whereas a locally assigned setting may
    // already be an array. Normalize both forms here, not in generic environment import.
    // These are borrowed color-option words, never shell commands or expansions.
    #[cfg(windows)]
    let words = arguments.iter().flat_map(|argument| {
        argument
            .as_char_slice()
            .split(|character| character.is_ascii_whitespace())
            .filter(|word| !word.is_empty())
            .map(wstr::from_char_slice)
    });
    let mut argv: Vec<&wstr> = Some(L!("")).into_iter().chain(words).collect();
    use ParsedArgs::*;
    match parse_text_face_and_options_with_color_support(&mut argv, false, support) {
        Ok(SetFace(specified_face)) => specified_face,
        Err(_) => Default::default(),
        Ok(PrintColors(_)) | Ok(PrintHelp) => unreachable!(),
    }
}

pub(crate) struct PrintColorsArgs<'argarray, 'args> {
    pub(crate) fg_args: &'argarray [&'args wstr],
    pub(crate) bg: Option<Color>,
    pub(crate) underline_color: Option<Color>,
    pub(crate) style: TextStyling,
}

pub(crate) enum ParsedArgs<'argarray, 'args> {
    SetFace(SpecifiedTextFace),
    PrintHelp,
    PrintColors(PrintColorsArgs<'argarray, 'args>),
}

pub(crate) enum ParseError<'args> {
    MissingOptArg,
    UnexpectedOptArg(usize),
    InvalidOptArg(&'static wstr, &'args wstr),
    UnknownColor(&'args wstr),
    UnknownUnderlineStyle(&'args wstr),
    UnknownOption(usize),
    InvalidFgArgCombination,
    InvalidFgPrintColorCombination,
}

fn parse_resettable_style<'a>(w: &WGetopter<'_, 'a, '_>) -> Result<ResettableStyle, &'a wstr> {
    let Some(arg) = w.woptarg else {
        return Ok(ResettableStyle::On(()));
    };
    if arg == "off" {
        Ok(ResettableStyle::Off)
    } else if arg == "on" {
        Ok(ResettableStyle::On(()))
    } else {
        Err(arg)
    }
}

pub(crate) fn parse_text_face_and_options<'argarray, 'args>(
    argv: &'argarray mut [&'args wstr],
    is_builtin: bool,
) -> Result<ParsedArgs<'argarray, 'args>, ParseError<'args>> {
    parse_text_face_and_options_with_color_support(argv, is_builtin, default_color_support())
}

fn parse_text_face_and_options_with_color_support<'argarray, 'args>(
    argv: &'argarray mut [&'args wstr],
    is_builtin: bool,
    support: ColorSupport,
) -> Result<ParsedArgs<'argarray, 'args>, ParseError<'args>> {
    let builtin_extra_args = if is_builtin { 0 } else { "hc".len() };
    let short_options = L!("f:b:oi::dr::s::u::ch");
    let short_options = &short_options[..short_options.len() - builtin_extra_args];
    let long_options: &[WOption] = &[
        wopt(L!("foreground"), ArgType::RequiredArgument, 'f'),
        wopt(L!("background"), ArgType::RequiredArgument, 'b'),
        wopt(L!("underline-color"), ArgType::RequiredArgument, '\x02'),
        wopt(L!("bold"), ArgType::NoArgument, 'o'),
        wopt(L!("underline"), ArgType::OptionalArgument, 'u'),
        wopt(L!("italics"), ArgType::OptionalArgument, 'i'),
        wopt(L!("dim"), ArgType::NoArgument, 'd'),
        wopt(L!("reverse"), ArgType::OptionalArgument, 'r'),
        wopt(L!("strikethrough"), ArgType::OptionalArgument, 's'),
        wopt(L!("theme"), ArgType::RequiredArgument, '\x01'),
        wopt(L!("reset"), ArgType::NoArgument, '\x03'),
        wopt(L!("help"), ArgType::NoArgument, 'h'),
        wopt(L!("print-colors"), ArgType::NoArgument, 'c'),
    ];
    let long_options = &long_options[..long_options.len() - builtin_extra_args];

    use ParseError::*;
    use ParsedArgs::*;

    let parse_color = |color_str| match Color::from_wstr(color_str) {
        color @ Some(_) => Ok(color),
        None => {
            if is_builtin {
                Err(UnknownColor(color_str))
            } else {
                Ok(None)
            }
        }
    };

    let mut fg_colors = vec![];
    let mut bg_colors = vec![];
    let mut underline_colors = vec![];
    let mut style = TextStyling::unknown();
    let mut print_color_mode = false;
    let mut reset = false;

    let mut w = WGetopter::new(short_options, long_options, argv);
    while let Some(c) = w.next_opt() {
        match c {
            'f' => {
                if let Some(fg) = parse_color(w.woptarg.unwrap())? {
                    fg_colors.push(fg);
                }
            }
            'b' => {
                if let Some(bg) = parse_color(w.woptarg.unwrap())? {
                    bg_colors.push(bg);
                }
            }
            '\x01' => (),
            '\x02' => {
                if let Some(underline_color) = parse_color(w.woptarg.unwrap())? {
                    underline_colors.push(underline_color);
                }
            }
            '\x03' => {
                reset = true;
            }
            'h' => {
                assert!(is_builtin);
                return Ok(PrintHelp);
            }
            'o' => style.bold = true,
            'i' => {
                style.italics =
                    parse_resettable_style(&w).map_err(|v| InvalidOptArg(L!("--italics"), v))?;
            }
            'd' => style.dim = true,
            'r' => {
                style.reverse =
                    parse_resettable_style(&w).map_err(|v| InvalidOptArg(L!("--reverse"), v))?;
            }
            's' => {
                style.strikethrough = parse_resettable_style(&w)
                    .map_err(|v| InvalidOptArg(L!("--strikethrough"), v))?;
            }
            'u' => {
                let arg = w.woptarg.unwrap_or(L!("single"));
                style.underline_style = if arg == "single" {
                    ResettableStyle::On(UnderlineStyle::Single)
                } else if arg == "double" {
                    ResettableStyle::On(UnderlineStyle::Double)
                } else if arg == "curly" {
                    ResettableStyle::On(UnderlineStyle::Curly)
                } else if arg == "dotted" {
                    ResettableStyle::On(UnderlineStyle::Dotted)
                } else if arg == "dashed" {
                    ResettableStyle::On(UnderlineStyle::Dashed)
                } else if arg == "off" {
                    ResettableStyle::Off
                } else {
                    return Err(UnknownUnderlineStyle(arg));
                };
            }
            'c' => {
                assert!(is_builtin);
                print_color_mode = true;
            }
            ':' => {
                return Err(MissingOptArg);
            }
            ';' => {
                return Err(UnexpectedOptArg(w.wopt_index - 1));
            }
            '?' => {
                return Err(UnknownOption(w.wopt_index - 1));
            }
            _ => unreachable!("unexpected retval from WGetopter"),
        }
    }

    let fg_args = &w.argv[w.wopt_index..];
    if !fg_args.is_empty() && !fg_colors.is_empty() {
        return Err(InvalidFgArgCombination);
    }

    let best_color = |colors: Vec<Color>| best_color(colors.into_iter(), support);

    let bg = best_color(bg_colors);
    let underline_color = best_color(underline_colors);

    if print_color_mode {
        if !fg_colors.is_empty() {
            return Err(InvalidFgPrintColorCombination);
        }
        return Ok(PrintColors(PrintColorsArgs {
            fg_args,
            bg,
            underline_color,
            style,
        }));
    }

    // Historical behavior: reset only applies if it's the first argument.
    if is_builtin && fg_args.first().is_some_and(|fg| fg == "reset") {
        return Ok(SetFace(SpecifiedTextFace {
            reset: true,
            ..Default::default()
        }));
    }

    fg_colors.reserve(fg_args.len());
    for fg in fg_args {
        if is_builtin && fg == "reset" {
            continue;
        }
        if let Some(fg) = parse_color(fg)? {
            if fg == Color::Normal {
                reset = true;
            }
            fg_colors.push(fg);
        }
    }
    // #1323: We may have multiple foreground colors. Choose the best one.
    let fg = best_color(fg_colors);
    Ok(SetFace(SpecifiedTextFace {
        fg,
        bg,
        underline_color,
        style,
        reset,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        ColorSupport, SpecifiedTextFace, TextStyling, parse_text_face,
        parse_text_face_with_color_support,
    };
    use fish_color::{Color, Color24};

    #[test]
    fn test_parse_text_face() {
        assert_eq!(
            parse_text_face(&["0000ee".into(), "--theme=default".into()]),
            SpecifiedTextFace {
                fg: Some(Color::Rgb(Color24 {
                    r: 0,
                    g: 0,
                    b: 0xee
                })),
                bg: None,
                underline_color: None,
                style: TextStyling::unknown(),
                reset: false
            }
        );
    }

    #[test]
    fn configured_color_depth_selects_named_fallback_without_losing_styles() {
        let arguments = ["ff0000".into(), "green".into(), "--bold".into()];
        let palette = parse_text_face_with_color_support(&arguments, ColorSupport::default());
        assert_eq!(palette.fg, Some(Color::Named { idx: 2 }));
        assert!(palette.style.bold);
        let rgb = parse_text_face_with_color_support(
            &arguments,
            ColorSupport {
                term_256color: true,
                term_24bit: true,
            },
        );
        assert_eq!(rgb.fg, Some(Color::from_rgb(255, 0, 0)));
        assert!(rgb.style.bold);
    }

    #[cfg(windows)]
    #[test]
    fn native_exported_color_expression_preserves_rgb_and_styles() {
        let support = ColorSupport {
            term_256color: true,
            term_24bit: true,
        };
        let scalar = parse_text_face_with_color_support(
            &[" \t--bold  123456 --underline=curly\n".into()],
            support,
        );
        assert_eq!(scalar.fg, Some(Color::from_rgb(18, 52, 86)));
        assert!(scalar.style.bold);
        assert_eq!(
            scalar.style.underline_style,
            super::ResettableStyle::On(super::UnderlineStyle::Curly),
        );
        let array = parse_text_face_with_color_support(
            &["--bold".into(), "123456".into(), "--underline=curly".into()],
            support,
        );
        assert_eq!(scalar, array);
    }
}
