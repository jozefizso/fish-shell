mod shared;

#[cfg(unix)]
pub mod abbr;
#[cfg(unix)]
pub mod argparse;
#[cfg(unix)]
pub mod bg;
#[cfg(unix)]
pub mod bind;
#[cfg(unix)]
pub mod block;
#[cfg(unix)]
pub mod r#break;
#[cfg(unix)]
pub mod breakpoint;
#[cfg(unix)]
pub mod builtin;
pub mod cd;
pub mod command;
#[cfg(unix)]
pub mod commandline;
#[cfg(unix)]
pub mod complete;
#[cfg(unix)]
pub mod contains;
#[cfg(unix)]
pub mod r#continue;
#[cfg(unix)]
pub mod count;
#[cfg(unix)]
pub mod disown;
pub mod echo;
#[cfg(unix)]
pub mod emit;
#[cfg(unix)]
pub mod eval;
pub mod exit;
pub mod r#false;
#[cfg(unix)]
pub mod fg;
#[cfg(unix)]
pub mod fish_indent;
#[cfg(unix)]
pub mod fish_key_reader;
#[cfg(unix)]
pub mod function;
#[cfg(unix)]
pub mod functions;
#[cfg(unix)]
pub mod r#gettext;
#[cfg(unix)]
pub mod history;
#[cfg(unix)]
pub mod jobs;
pub mod math;
pub mod path;
pub mod printf;
pub mod pwd;
#[cfg(unix)]
pub mod random;
#[cfg(unix)]
pub mod read;
pub mod realpath;
pub mod r#return;
pub mod set;
#[cfg(unix)]
pub mod set_color;
#[cfg(unix)]
pub mod source;
#[cfg(unix)]
pub mod status;
#[cfg(unix)]
pub mod string;
pub mod test;
pub mod r#true;
pub mod r#type;
#[cfg(unix)]
pub mod ulimit;
#[cfg(unix)]
pub mod wait;

mod prelude {
    pub use super::shared::*;
    pub use libc::c_int;
    pub use std::borrow::Cow;

    #[allow(unused_imports)]
    pub(crate) use crate::{
        flog::{flog, flogf},
        io::{IoStreams, SeparationType},
        parser::Parser,
        prelude::*,
        wutil::{fish_wcstoi, fish_wcstol, fish_wcstoul},
    };
    pub(crate) use fish_wgetopt::{
        ArgType::{self, *},
        NON_OPTION_CHAR, WGetopter, WOption, wopt,
    };
}

pub use shared::*;
