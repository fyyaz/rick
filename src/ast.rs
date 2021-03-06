// -------------------------------------------------------------------------------------------------
// Rick, a Rust intercal compiler.  Save your souls!
//
// Copyright (c) 2015 Georg Brandl
//
// This program is free software; you can redistribute it and/or modify it under the terms of the
// GNU General Public License as published by the Free Software Foundation; either version 2 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
// even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with this program;
// if not, write to the Free Software Foundation, Inc., 675 Mass Ave, Cambridge, MA 02139, USA.
// -------------------------------------------------------------------------------------------------

/// Provides the Abstract Sadist Tree, for representing INTERCAL programs in memory.
///
/// Most of the stuff here should be pretty standard.

use std::collections::BTreeMap;
use std::default::Default;
use std::fmt::{ Display, Error, Formatter };

use err::RtError;
use lex::SrcLine;

/// A label
pub type Label = u16;
/// A logical line (a single statement).  A logical line can span multiple
/// source lines, or a source line can contain multiple logical lines!
pub type LogLine = u16;

/// A whole program, with meta-information used at eval-time.
#[derive(PartialEq, Eq, Debug)]
pub struct Program {
    /// Statements in the program.
    pub stmts: Vec<Stmt>,
    /// Maps label numbers to logical line (statement) numbers.
    pub labels: BTreeMap<Label, LogLine>,
    /// A list of statement types (for processing ABSTAINs), represented by the
    /// `Abstain` type that contains an enum item for every type.
    pub stmt_types: Vec<Abstain>,
    /// Info about variables in the program, by type: spot, twospot, tail, hybrid.
    pub var_info: (Vec<VarInfo>, Vec<VarInfo>, Vec<VarInfo>, Vec<VarInfo>),
    /// True if the program uses computed COME FROM.
    pub uses_complex_comefrom: bool,
    /// True if we added the syslib or floatlib to the program.
    pub added_syslib: bool,
    pub added_floatlib: bool,
    /// The line on which the compiler bug E774 should be triggered.
    /// If this is set to a number >= stmts.len(), the bug is disabled.
    pub bugline: LogLine,
}

/// A single statement.
#[derive(PartialEq, Eq, Debug)]
pub struct Stmt {
    pub body: StmtBody,
    pub props: StmtProps,
    // the next two properties are determined after parsing
    pub comefrom: Option<LogLine>,
    pub can_abstain: bool,
}

/// Common properties for all statements.
#[derive(PartialEq, Eq, Debug)]
pub struct StmtProps {
    /// Source line of the statement.
    pub srcline: SrcLine,
    /// Source line of the next statement (provides "on the way to") in error
    /// messages.
    pub onthewayto: SrcLine,
    /// Label of the line, or zero if no label.
    pub label: Label,
    /// Execution chance in %, usually 100.
    pub chance: u8,
    /// True if the statement is polite (programmer said PLEASE).
    pub polite: bool,
    /// True if the statement is initially abstained (NOT or DON'T).
    pub disabled: bool,
}

/// Type-of-statement dependent data.
#[derive(PartialEq, Eq, Debug)]
pub enum StmtBody {
    /// An undecodable statement ("splat"), resulting in a runtime error when
    /// executed (and not abstained).
    Error(RtError),
    /// A calculation ("gets") operation.
    Calc(Var, Expr),
    /// An array dimensioning operation (same syntax as Calc).
    Dim(Var, Vec<Expr>),
    DoNext(Label),
    ComeFrom(ComeFrom),
    Resume(Expr),
    Forget(Expr),
    Ignore(Vec<Var>),
    Remember(Vec<Var>),
    Stash(Vec<Var>),
    Retrieve(Vec<Var>),
    Abstain(Option<Expr>, Vec<Abstain>),
    Reinstate(Vec<Abstain>),
    WriteIn(Vec<Var>),
    ReadOut(Vec<Expr>),
    TryAgain,
    GiveUp,
    /// Print the given bytes.  Only used when the constant-output optimization
    /// kicks in and reduces the whole program to this statement.
    Print(Vec<u8>),
}

/// A variable reference (store or load).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Var {
    /// 16-bit "spot".
    I16(usize),
    /// 32-bit "twospot".
    I32(usize),
    /// 16-bit array "tail", with subscripts.
    A16(usize, Vec<Expr>),
    /// 32-bit array "hybrid", with subscripts.
    A32(usize, Vec<Expr>),
}

/// An expression.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Expr {
    /// A literal number.  In the source, this can only be 16-bit (enforced by
    /// the parser), but after optimizations we can end up with 32-bit values.
    Num(VType, u32),
    /// A variable reference.
    Var(Var),
    // INTERCAL operators
    Mingle(Box<Expr>, Box<Expr>),
    Select(VType, Box<Expr>, Box<Expr>),
    And(VType, Box<Expr>),
    Or(VType, Box<Expr>),
    Xor(VType, Box<Expr>),
    // only used after optimizing
    RsNot(Box<Expr>),
    RsAnd(Box<Expr>, Box<Expr>),
    RsOr(Box<Expr>, Box<Expr>),
    RsXor(Box<Expr>, Box<Expr>),
    RsRshift(Box<Expr>, Box<Expr>),
    RsLshift(Box<Expr>, Box<Expr>),
    // RsEqual(Box<Expr>, Box<Expr>),
    RsNotEqual(Box<Expr>, Box<Expr>),
    RsPlus(Box<Expr>, Box<Expr>),
    RsMinus(Box<Expr>, Box<Expr>),
    // RsTimes(Box<Expr>, Box<Expr>),
    // RsDivide(Box<Expr>, Box<Expr>),
    // RsModulus(Box<Expr>, Box<Expr>),
}

/// Type of an expression, used when the width actually matters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VType {
    I16,
    I32,
}

/// Specification of targets for an ABSTAIN or REINSTATE.
#[derive(PartialEq, Eq, Debug)]
pub enum Abstain {
    Label(Label),
    Calc,
    Next,
    Resume,
    Forget,
    Ignore,
    Remember,
    Stash,
    Retrieve,
    Abstain,
    Reinstate,
    ComeFrom,
    ReadOut,
    WriteIn,
    TryAgain,
}

/// Specification of the target for a COME FROM.
#[derive(PartialEq, Eq, Debug)]
pub enum ComeFrom {
    Label(Label),
    Expr(Expr),
    Gerund(Abstain),
}

/// Information about a variable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VarInfo {
    /// Variable is IGNOREd somewhere in the program.
    pub can_ignore: bool,
    /// Variable is STASHed somewhere in the program.
    pub can_stash: bool,
}


impl Stmt {
    /// Determine the abstain type for the statement. Label(0) is used as an
    /// escape value.
    pub fn stype(&self) -> Abstain {
        match self.body {
            StmtBody::Error(_) => Abstain::Label(0),
            StmtBody::Calc(..) => Abstain::Calc,
            StmtBody::Dim(..) => Abstain::Calc,
            StmtBody::DoNext(_) => Abstain::Next,
            StmtBody::ComeFrom(_) => Abstain::ComeFrom,
            StmtBody::Resume(_) => Abstain::Resume,
            StmtBody::Forget(_) => Abstain::Forget,
            StmtBody::Ignore(_) => Abstain::Ignore,
            StmtBody::Remember(_) => Abstain::Remember,
            StmtBody::Stash(_) => Abstain::Stash,
            StmtBody::Retrieve(_) => Abstain::Retrieve,
            StmtBody::Abstain(..) => Abstain::Abstain,
            StmtBody::Reinstate(_) => Abstain::Reinstate,
            StmtBody::WriteIn(_) => Abstain::WriteIn,
            StmtBody::ReadOut(_) => Abstain::ReadOut,
            StmtBody::TryAgain => Abstain::TryAgain,
            StmtBody::GiveUp => Abstain::Label(0),
            StmtBody::Print(_) => Abstain::Label(0),
        }
    }

    /// Synthesize a statement with default metadata.
    pub fn new_with(body: StmtBody) -> Stmt {
        Stmt { body: body, props: StmtProps::default(),
               comefrom: None, can_abstain: true }
    }
}

impl StmtBody {
    // helpers for Display
    fn fmt_pluslist<T: Display>(&self, vars: &Vec<T>) -> String {
        vars.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" + ")
    }

    fn fmt_bylist(&self, vars: &Vec<Expr>) -> String {
        vars.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" BY ")
    }
}

impl Expr {
    /// Get the variable width for this expression.  Defaults to 32-bit if the
    /// type of expression has no information about the width.
    pub fn get_vtype(&self) -> VType {
        match *self {
            Expr::Num(vtype, _) => vtype,
            Expr::And(vtype, _) | Expr::Or(vtype, _) | Expr::Xor(vtype, _) => vtype,
            Expr::Select(vtype, _, _) => vtype,
            Expr::Mingle(..) => VType::I32,
            Expr::RsAnd(..) | Expr::RsOr(..) | Expr::RsXor(..) |
            Expr::RsNot(..) | Expr::RsRshift(..) | Expr::RsLshift(..) |
            Expr::RsNotEqual(..) | Expr::RsMinus(..) |
            Expr::RsPlus(..) => VType::I32,
            Expr::Var(ref v) => v.get_vtype(),
        }
    }
}

impl Var {
    /// Is this Var a dimensioning access (array without subscript)?
    pub fn is_dim(&self) -> bool {
        match *self {
            Var::A16(_, ref v) if v.is_empty() => true,
            Var::A32(_, ref v) if v.is_empty() => true,
            _ => false,
        }
    }

    /// Get a unique key that identifies this variable among all var types.
    pub fn unique(&self) -> (u8, usize) {
        match *self {
            Var::I16(n)    => (0, n),
            Var::I32(n)    => (1, n),
            Var::A16(n, _) => (2, n),
            Var::A32(n, _) => (3, n),
        }
    }

    /// Rename the variable with a new number.
    pub fn rename(&mut self, new: usize) {
        match *self {
            Var::I16(ref mut n) |
            Var::I32(ref mut n) |
            Var::A16(ref mut n, _) |
            Var::A32(ref mut n, _) => {
                *n = new;
            }
        }
    }

    /// Get the VType for this Var.
    pub fn get_vtype(&self) -> VType {
        match *self {
            Var::I16(..) | Var::A16(..) => VType::I16,
            Var::I32(..) | Var::A32(..) => VType::I32,
        }
    }
}

impl VarInfo {
    pub fn new() -> VarInfo {
        VarInfo { can_ignore: true, can_stash: true }
    }
}


impl Default for StmtProps {
    fn default() -> StmtProps {
        StmtProps { label: 0,
                    srcline: 0,
                    onthewayto: 0,
                    chance: 100,
                    polite: false,
                    disabled: false, }
    }
}


// Display implementation to be able to pretty-print parts of an AST.

impl Display for Program {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        for stmt in &self.stmts {
            try!(write!(fmt, "{}\n", stmt));
        }
        Ok(())
    }
}

impl Display for Stmt {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        try!(write!(fmt, "#{:03}  ", self.props.srcline));
        if self.props.label > 0 {
            try!(write!(fmt, "({:5}) ", self.props.label));
        } else {
            try!(write!(fmt, "        "));
        }
        if self.props.polite {
            try!(write!(fmt, "PLEASE "));
        } else {
            try!(write!(fmt, "DO     "));
        }
        if self.props.disabled {
            try!(write!(fmt, "NOT "));
        } else {
            try!(write!(fmt, "    "));
        }
        if self.props.chance < 100 {
            try!(write!(fmt, "%{} ", self.props.chance));
        }
        write!(fmt, "{}", self.body)
    }
}

impl Display for StmtBody {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            StmtBody::Error(ref err) => write!(fmt, "* {}", err.short_string()),
            StmtBody::Calc(ref var, ref expr) => write!(fmt, "{} <- {}", var, expr),
            StmtBody::Dim(ref var, ref exprs) => write!(fmt, "{} <- {}", var,
                                                        self.fmt_bylist(exprs)),
            StmtBody::DoNext(ref line) => write!(fmt, "({}) NEXT", line),
            StmtBody::ComeFrom(ref spec) => write!(fmt, "COME FROM {}", spec),
            StmtBody::Resume(ref expr) => write!(fmt, "RESUME {}", expr),
            StmtBody::Forget(ref expr) => write!(fmt, "FORGET {}", expr),
            StmtBody::Ignore(ref vars) => write!(fmt, "IGNORE {}", self.fmt_pluslist(vars)),
            StmtBody::Remember(ref vars) => write!(fmt, "REMEMBER {}", self.fmt_pluslist(vars)),
            StmtBody::Stash(ref vars) => write!(fmt, "STASH {}", self.fmt_pluslist(vars)),
            StmtBody::Retrieve(ref vars) => write!(fmt, "RETRIEVE {}", self.fmt_pluslist(vars)),
            StmtBody::Abstain(ref expr, ref whats) => match *expr {
                None => write!(fmt, "ABSTAIN FROM {}", self.fmt_pluslist(whats)),
                Some(ref e) => write!(fmt, "ABSTAIN {} FROM {}", e, self.fmt_pluslist(whats)),
            },
            StmtBody::Reinstate(ref whats) => write!(fmt, "REINSTATE {}", self.fmt_pluslist(whats)),
            StmtBody::WriteIn(ref vars) => write!(fmt, "WRITE IN {}", self.fmt_pluslist(vars)),
            StmtBody::ReadOut(ref vars) => write!(fmt, "READ OUT {}", self.fmt_pluslist(vars)),
            StmtBody::TryAgain => write!(fmt, "TRY AGAIN"),
            StmtBody::GiveUp => write!(fmt, "GIVE UP"),
            StmtBody::Print(_) => write!(fmt, "<PRINT>"),
        }
    }
}

impl Display for Var {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Var::I16(n) => write!(fmt, ".{}", n),
            Var::I32(n) => write!(fmt, ":{}", n),
            Var::A16(n, ref subs) => {
                try!(write!(fmt, ",{}", n));
                for sub in subs {
                    try!(write!(fmt, " SUB {}", sub));
                }
                Ok(())
            }
            Var::A32(n, ref subs) => {
                try!(write!(fmt, ";{}", n));
                for sub in subs {
                    try!(write!(fmt, " SUB {}", sub));
                }
                Ok(())
            }
        }
    }
}

impl Display for Expr {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Expr::Num(_, ref n) => write!(fmt, "#{:X}", n),
            Expr::Var(ref v) => v.fmt(fmt),
            Expr::Mingle(ref x, ref y) => write!(fmt, "({} $ {})", x, y),
            Expr::Select(_, ref x, ref y) => write!(fmt, "({} ~ {})", x, y),
            Expr::And(t, ref x) => write!(fmt, "&{} {}",
                                          if t == VType::I16 { "16" } else { "32" }, x),
            Expr::Or(t, ref x) => write!(fmt, "V{} {}",
                                         if t == VType::I16 { "16" } else { "32" }, x),
            Expr::Xor(t, ref x) => write!(fmt, "?{} {}",
                                          if t == VType::I16 { "16" } else { "32" }, x),
            // optimized exprs
            Expr::RsNot(ref x) => write!(fmt, "!{}", x),
            Expr::RsAnd(ref x, ref y) => write!(fmt, "({} & {})", x, y),
            Expr::RsOr(ref x, ref y) => write!(fmt, "({} | {})", x, y),
            Expr::RsXor(ref x, ref y) => write!(fmt, "({} ^ {})", x, y),
            Expr::RsRshift(ref x, ref y) => write!(fmt, "({} >> {})", x, y),
            Expr::RsLshift(ref x, ref y) => write!(fmt, "({} << {})", x, y),
            // Expr::RsEqual(ref x, ref y) => write!(fmt, "({} == {})", x, y),
            Expr::RsNotEqual(ref x, ref y) => write!(fmt, "({} != {})", x, y),
            Expr::RsPlus(ref x, ref y) => write!(fmt, "({} + {})", x, y),
            Expr::RsMinus(ref x, ref y) => write!(fmt, "({} - {})", x, y),
        }
    }
}

impl Display for Abstain {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Abstain::Label(n) => write!(fmt, "({})", n),
            Abstain::Calc => write!(fmt, "CALCULATING"),
            Abstain::Next => write!(fmt, "NEXTING"),
            Abstain::Resume => write!(fmt, "RESUMING"),
            Abstain::Forget => write!(fmt, "FORGETTING"),
            Abstain::Ignore => write!(fmt, "IGNORING"),
            Abstain::Remember => write!(fmt, "REMEMBERING"),
            Abstain::Stash => write!(fmt, "STASHING"),
            Abstain::Retrieve => write!(fmt, "RETRIEVING"),
            Abstain::Abstain => write!(fmt, "ABSTAINING"),
            Abstain::Reinstate => write!(fmt, "REINSTATING"),
            Abstain::ComeFrom => write!(fmt, "COMING FROM"),
            Abstain::ReadOut => write!(fmt, "READING OUT"),
            Abstain::WriteIn => write!(fmt, "WRITING IN"),
            Abstain::TryAgain => write!(fmt, "TRYING AGAIN"),
        }
    }
}

impl Display for ComeFrom {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            ComeFrom::Label(n) => write!(fmt, "({})", n),
            ComeFrom::Expr(ref e) => write!(fmt, "{}", e),
            ComeFrom::Gerund(ref g) => write!(fmt, "{}", g),
        }
    }
}
