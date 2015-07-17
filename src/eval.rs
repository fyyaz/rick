// -------------------------------------------------------------------------------------------------
// Rick, a Rust intercal interpreter.  Save your souls!
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

use std::collections::HashSet;
use std::default::Default;
use std::rc::Rc;

use ast;
use err;
use util::{ write_number, read_number, check_chance,
            mingle, select, and_16, and_32, or_16, or_32, xor_16, xor_32 };


#[derive(Clone)]
struct Array<T>(pub Vec<u16>, pub Vec<T>);

impl<T: Clone + Default> Array<T> {
    pub fn new(subs: Vec<u16>) -> Array<T> {
        let total = subs.iter().product::<u16>() as usize;
        let value = Default::default();
        Array(subs, vec![value; total])
    }

    pub fn empty() -> Array<T> {
        Array(vec![], vec![])
    }
}

#[derive(Clone)]
struct Var<T> {
    pub val: T,
    pub stack: Vec<T>,
}

impl<T> Var<T> {
    pub fn new(t: T) -> Var<T> {
        Var { val: t, stack: Vec::new() }
    }
}

pub struct Eval {
    program: Rc<ast::Program>,
    spot: Vec<Var<u16>>,
    twospot: Vec<Var<u32>>,
    tail: Vec<Var<Array<u16>>>,
    hybrid: Vec<Var<Array<u32>>>,
    ignored: HashSet<(u8, u16)>,
    jumps: Vec<ast::LogLine>,
    abstentions: Vec<bool>,
    _in_state: u8,
    _out_state: u8,
}

type EvalRes<T> = Result<T, err::Error>;

enum StmtRes {
    Next,
    Jump(usize),
    Back(usize),
    End,
}

impl Eval {
    pub fn new(program: ast::Program) -> Eval {
        let mut abs = Vec::new();
        for stmt in &program.stmts {
            abs.push(stmt.props.disabled);
        }
        let nv = program.n_vars;
        Eval {
            program: Rc::new(program),
            spot:    vec![Var::new(0); nv.0 as usize],
            twospot: vec![Var::new(0); nv.1 as usize],
            tail:    vec![Var::new(Array::empty()); nv.2 as usize],
            hybrid:  vec![Var::new(Array::empty()); nv.3 as usize],
            ignored: HashSet::new(),
            jumps:   Vec::new(),
            abstentions: abs,
            _in_state: 0,
            _out_state: 0,
        }
    }

    pub fn eval(&mut self) -> EvalRes<()> {
        let mut pctr = 0;  // index of current statement
        let program = self.program.clone();
        let nstmts = program.stmts.len();
        loop {
            // check for falling off the end
            if pctr >= nstmts {
                return Err(err::with_line(&err::IE663, nstmts as usize));
            }
            // execute statement if not abstained
            if !self.abstentions[pctr] {
                let stmt = &program.stmts[pctr];
                // check execution chance
                if check_chance(stmt.props.chance) {
                    let res = match self.eval_stmt(stmt) {
                        Err(mut err) => {
                            err.set_line(stmt.props.srcline);
                            return Err(err);
                        },
                        Ok(res)  => res
                    };
                    match res {
                        StmtRes::Jump(n) => {
                            self.jumps.push(pctr as u16);  // push the line with the NEXT
                            pctr = n;
                            continue;  // do not check for COME FROMs
                        },
                        StmtRes::Back(n) => {
                            pctr = n;  // will be incremented below after COME FROM check
                        }
                        StmtRes::End     => break,
                        StmtRes::Next    => { },
                    }
                }
            }
            // check for COME FROMs from this line
            let lbl = self.program.stmts[pctr].props.label;
            if lbl > 0 {
                if let Some(next) = self.program.comefroms.get(&lbl) {
                    // check for abstained COME FROM
                    if !self.abstentions[*next as usize] {
                        pctr = *next as usize;
                        continue;
                    }
                }
            }
            // no COME FROM, normal execution
            pctr += 1;
        }
        Ok(())
    }

    /// Process a single statement.
    fn eval_stmt(&mut self, stmt: &ast::Stmt) -> EvalRes<StmtRes> {
        //println!("        {}", stmt);
        match &stmt.body {
            &ast::StmtBody::GiveUp => Ok(StmtRes::End),
            &ast::StmtBody::Error(ref e) => Err((*e).clone()),
            &ast::StmtBody::Calc(ref var, ref expr) => {
                let val = try!(self.eval_expr(expr));
                try!(self.assign(var, val));
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Dim(ref var, ref exprs) => {
                let vals = try!(self.eval_exprlist(exprs));
                try!(self.array_dim(var, vals));
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::DoNext(n) => {
                match self.program.labels.get(&n) {
                    Some(i) => {
                        if self.jumps.len() >= 80 {
                            return Err(err::new(&err::IE123))
                        }
                        Ok(StmtRes::Jump(*i as usize))
                    },
                    None => {
                        Err(err::new(&err::IE129))
                    }
                }
            }
            &ast::StmtBody::ComeFrom(_) => {
                // nothing to do here at runtime
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Resume(ref expr) => {
                let n = try!(self.eval_expr(expr)).as_u32();
                let next = try!(self.pop_jumps(n));
                Ok(StmtRes::Back(next as usize))
            }
            &ast::StmtBody::Forget(ref expr) => {
                let n = try!(self.eval_expr(expr)).as_u32();
                try!(self.pop_jumps(n));
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Ignore(ref vars) => {
                for var in vars {
                    try!(self.set_rw(var, false));
                }
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Remember(ref vars) => {
                for var in vars {
                    try!(self.set_rw(var, true));
                }
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Stash(ref vars) => {
                for var in vars {
                    try!(self.stash(var));
                }
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Retrieve(ref vars) => {
                for var in vars {
                    try!(self.retrieve(var));
                }
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Abstain(ref what) => {
                try!(self.abstain(what, true));
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::Reinstate(ref what) => {
                try!(self.abstain(what, false));
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::ReadOut(ref vars) => {
                for var in vars {
                    let val = match var {
                        &ast::Readout::Const(n) => n as u32,
                        &ast::Readout::Var(ref v) => {
                            let varval = try!(self.lookup(v));
                            varval.as_u32()
                        }
                    };
                    write_number(val);
                }
                Ok(StmtRes::Next)
            }
            &ast::StmtBody::WriteIn(ref var) => {
                let n = try!(read_number());
                try!(self.assign(var, ast::Val::from_u32(n)));
                Ok(StmtRes::Next)
            }
        }
    }

    /// Evaluate an expression to a value.
    fn eval_expr(&self, expr: &ast::Expr) -> EvalRes<ast::Val> {
        match *expr {
            ast::Expr::Num(ref n) => Ok(n.clone()),
            ast::Expr::Var(ref var) => self.lookup(var),
            ast::Expr::Mingle(ref vx, ref wx) => {
                let v = try!(self.eval_expr(vx));
                let w = try!(self.eval_expr(wx));
                mingle(v.as_u32(), w.as_u32()).map(ast::Val::I32)
            },
            ast::Expr::Select(ref vx, ref wx) => {
                let v = try!(self.eval_expr(vx));
                let w = try!(self.eval_expr(wx));
                select(v.as_u32(), w.as_u32()).map(ast::Val::I32)
            },
            ast::Expr::And(ref vx) => {
                let v = try!(self.eval_expr(vx));
                match v {
                    ast::Val::I16(v) => Ok(ast::Val::I16(and_16(v))),
                    ast::Val::I32(v) => Ok(ast::Val::I32(and_32(v))),
                }
            },
            ast::Expr::Or(ref vx) => {
                let v = try!(self.eval_expr(vx));
                match v {
                    ast::Val::I16(v) => Ok(ast::Val::I16(or_16(v))),
                    ast::Val::I32(v) => Ok(ast::Val::I32(or_32(v))),
                }
            },
            ast::Expr::Xor(ref vx) => {
                let v = try!(self.eval_expr(vx));
                match v {
                    ast::Val::I16(v) => Ok(ast::Val::I16(xor_16(v))),
                    ast::Val::I32(v) => Ok(ast::Val::I32(xor_32(v))),
                }
            },
        }
    }

    fn eval_exprlist(&self, exprs: &Vec<ast::Expr>) -> EvalRes<Vec<ast::Val>> {
        exprs.iter().map(|v| self.eval_expr(v)).collect::<Result<Vec<_>, _>>()
    }

    /// Assign to a variable.
    fn assign(&mut self, var: &ast::Var, val: ast::Val) -> EvalRes<()> {
        //println!("assign: {:?} = {}", var, val.as_u32());
        if self.ignored.contains(&var.unique()) {
            return Ok(());
        }
        match *var {
            ast::Var::I16(n) => {
                let vent = &mut self.spot[n as usize];
                vent.val = try!(val.as_u16());
                Ok(())
            },
            ast::Var::I32(n) => {
                let vent = &mut self.twospot[n as usize];
                vent.val = val.as_u32();
                Ok(())
            },
            ast::Var::A16(n, ref subs) => {
                let subs = try!(self.eval_exprlist(subs));
                let val = try!(val.as_u16());
                Eval::array_assign(&mut self.tail, n, subs, val)
            },
            ast::Var::A32(n, ref subs) => {
                let subs = try!(self.eval_exprlist(subs));
                Eval::array_assign(&mut self.hybrid, n, subs, val.as_u32())
            },
        }
    }

    /// Dimension an array.
    fn array_dim(&mut self, var: &ast::Var, vals: Vec<ast::Val>) -> EvalRes<()> {
        if self.ignored.contains(&var.unique()) {
            return Ok(());
        }
        let vals = try!(vals.iter().map(|v| v.as_u16()).collect::<Result<Vec<_>, _>>());
        match *var {
            ast::Var::A16(n, _) => {
                Eval::array_dimension(&mut self.tail, n, vals)
            },
            ast::Var::A32(n, _) => {
                Eval::array_dimension(&mut self.hybrid, n, vals)
            },
            _ => unimplemented!()
        }
    }

    /// Pop "n" jumps from the jump stack and return the last one.
    fn pop_jumps(&mut self, n: u32) -> EvalRes<u16> {
        if n == 0 {
            return Err(err::new(&err::IE621));
        }
        if self.jumps.len() < n as usize {
            return Err(err::new(&err::IE632));
        }
        let newlen = self.jumps.len() - (n as usize - 1);
        self.jumps.truncate(newlen);
        Ok(self.jumps.pop().unwrap())
    }

    /// Look up the value of a variable.
    fn lookup(&self, var: &ast::Var) -> EvalRes<ast::Val> {
        match *var {
            ast::Var::I16(n) => {
                Ok(ast::Val::I16(self.spot[n as usize].val))
            },
            ast::Var::I32(n) => {
                Ok(ast::Val::I32(self.twospot[n as usize].val))
            },
            ast::Var::A16(n, ref subs) => {
                let subs = try!(self.eval_exprlist(subs));
                Eval::array_lookup(&self.tail, n, subs).map(ast::Val::I16)
            },
            ast::Var::A32(n, ref subs) => {
                let subs = try!(self.eval_exprlist(subs));
                Eval::array_lookup(&self.hybrid, n, subs).map(ast::Val::I32)
            },
        }
    }

    /// Process a STASH statement.
    fn stash(&mut self, var: &ast::Var) -> EvalRes<()> {
        match *var {
            ast::Var::I16(n) => {
                let vent = &mut self.spot[n as usize];
                vent.stack.push(vent.val);
            },
            ast::Var::I32(n) => {
                let vent = &mut self.twospot[n as usize];
                vent.stack.push(vent.val);
            },
            ast::Var::A16(n, _) => {
                let vent = &mut self.tail[n as usize];
                vent.stack.push(vent.val.clone());
            }
            ast::Var::A32(n, _) => {
                let vent = &mut self.hybrid[n as usize];
                vent.stack.push(vent.val.clone());
            }
        }
        Ok(())
    }

    /// Process a RETRIEVE statement.
    fn retrieve(&mut self, var: &ast::Var) -> EvalRes<()> {
        match *var {
            ast::Var::I16(n) => Eval::generic_retrieve(&mut self.spot, n),
            ast::Var::I32(n) => Eval::generic_retrieve(&mut self.twospot, n),
            ast::Var::A16(n, _) => Eval::generic_retrieve(&mut self.tail, n),
            ast::Var::A32(n, _) => Eval::generic_retrieve(&mut self.hybrid, n),
        }
    }

    /// Process an IGNORE or REMEMBER statement.
    fn set_rw(&mut self, var: &ast::Var, rw: bool) -> EvalRes<()> {
        if rw {
            self.ignored.remove(&var.unique());
        } else {
            self.ignored.insert(var.unique());
        }
        Ok(())
    }

    /// Process an ABSTAIN or REINSTATE statement.
    fn abstain(&mut self, what: &ast::Abstain, abstain: bool) -> EvalRes<()> {
        if let &ast::Abstain::Line(n) = what {
            if !self.program.labels.contains_key(&n) {
                return Err(err::new(&err::IE139));
            }
            let idx = self.program.labels[&n];
            self.abstentions[idx as usize] = abstain;
        } else {
            for (i, stype) in self.program.stmt_types.iter().enumerate() {
                if stype == what {
                    self.abstentions[i] = abstain;
                }
            }
        }
        Ok(())
    }

    /// Generic array dimension helper.
    fn array_dimension<T: Clone + Default>(map: &mut Vec<Var<Array<T>>>, n: u16,
                                           subs: Vec<u16>) -> EvalRes<()> {
        let mut vent = &mut map[n as usize];
        vent.val = Array::new(subs);
        Ok(())
    }

    /// Generic array assignment helper.
    fn array_assign<T>(map: &mut Vec<Var<Array<T>>>, n: u16,
                       subs: Vec<ast::Val>, val: T) -> EvalRes<()> {
        let subs = try!(subs.iter().map(|v| v.as_u16()).collect::<Result<Vec<_>, _>>());
        let vent = &mut map[n as usize];
        if subs.len() != vent.val.0.len() {
            return Err(err::new(&err::IE241));
        }
        let mut ix = 0;
        let mut prev_dim = 1;
        for (sub, dim) in subs.iter().zip(&vent.val.0) {
            if *sub > *dim {
                return Err(err::new(&err::IE241));
            }
            ix += (sub - 1) * prev_dim;
            prev_dim = *dim;
        }
        //println!("array assign: dim={:?} subs={:?} ix={}", vent.val.0, subs, ix);
        vent.val.1[ix as usize] = val;
        Ok(())
    }

    /// Generic array lookup helper.
    fn array_lookup<T: Copy>(map: &Vec<Var<Array<T>>>, n: u16,
                             subs: Vec<ast::Val>) -> EvalRes<T> {
        let subs = try!(subs.iter().map(|v| v.as_u16()).collect::<Result<Vec<_>, _>>());
        let vent = &map[n as usize];
        if subs.len() != vent.val.0.len() {
            return Err(err::new(&err::IE241));
        }
        let mut ix = 0;
        let mut prev_dim = 1;
        for (sub, dim) in subs.iter().zip(&vent.val.0) {
            if *sub > *dim {
                return Err(err::new(&err::IE241));
            }
            ix += (sub - 1) * prev_dim;
            prev_dim = *dim;
        }
        Ok(vent.val.1[ix as usize])
    }

    /// Generic RETRIEVE helper.
    fn generic_retrieve<T>(map: &mut Vec<Var<T>>, n: u16) -> EvalRes<()> {
        let vent = &mut map[n as usize];
        match vent.stack.pop() {
            None => Err(err::new(&err::IE436)),
            Some(v) => {
                vent.val = v;
                Ok(())
            }
        }
    }
}
