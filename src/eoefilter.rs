use std::sync::Arc;

use super::{EachOrEvery, eachorevery::EachOrEveryIndex};

fn un_rle<F>(input: &[(usize,usize)], cb: F) -> Arc<Vec<usize>> where F: Fn(usize) -> usize {
    let mut out = vec![];
    for (start,len) in input {
        for i in *start..(*start+*len) {
            out.push(cb(i));
        }
    }
    Arc::new(out)
}

struct NumIterator<'a> {
    filter: &'a [(usize,usize)],
    range_index: usize,
    pos: usize
}

impl<'a> NumIterator<'a> {
    fn new(filter: &'a [(usize,usize)]) -> NumIterator<'a> {
        NumIterator { filter, range_index: 0, pos: 0 }
    }

    fn peek(&mut self) -> Option<usize> {
        loop {
            if self.range_index >= self.filter.len() { return None; }
            if self.pos < self.filter[self.range_index].1 { break; }
            self.pos = 0;
            self.range_index += 1;
        }
        Some(self.filter[self.range_index].0 + self.pos)
    }

    fn advance(&mut self, index: usize) {
        loop {
            if self.range_index >= self.filter.len() { return; }
            let range = &self.filter[self.range_index];
            if index < range.0 + range.1 {
                self.pos = if index > range.0 { index - range.0 } else { 0 };
                return;
            }
            self.pos = 0;
            self.range_index += 1;
        }
    }
}


// XXX run-length
pub struct EachOrEveryFilterBuilder(Vec<(usize,usize)>,usize);

impl EachOrEveryFilterBuilder {
    pub fn new() -> EachOrEveryFilterBuilder { EachOrEveryFilterBuilder(vec![],0) }

    pub fn set(&mut self, index: usize) {
        self.1 += 1;
        if let Some((last_index,last_len)) = self.0.last_mut() {
            if *last_index + *last_len == index {
                *last_len += 1;
                return;
            }
        }
        self.0.push((index,1));
    }

    pub fn make(self, len: usize) -> EachOrEveryFilter {
        if self.0.len() == 0 {
            EachOrEveryFilter::none(len)
        } else {
            if self.0.len() == 1 {
                if self.0[0].0 == 0 && self.0[0].1 == len {
                    return EachOrEveryFilter::all(len);
                }
            }
            EachOrEveryFilter {
                data: EachOrEveryFilterData::Some(self.0),
                len, count: self.1
            }
        }
    }
}

fn union(a: &[(usize,usize)], b: &[(usize,usize)],len: usize) -> EachOrEveryFilter {
    let mut a_iter = NumIterator::new(a);
    let mut b_iter = NumIterator::new(b);
    let mut out = EachOrEveryFilterBuilder::new();
    loop {
        match (a_iter.peek(),b_iter.peek()) {
            (Some(a),Some(b)) => {
                if a == b { 
                    out.set(a);
                    a_iter.advance(a+1); 
                    b_iter.advance(b+1);
                } else if a < b {
                    out.set(a);
                    a_iter.advance(a+1);
                } else if a > b {
                    out.set(b);
                    b_iter.advance(b+1);
                }
            },
            (Some(a),None) => {
                out.set(a);
                a_iter.advance(a+1);
            },
            (None,Some(b)) => {
                out.set(b);
                b_iter.advance(b+1);
            },
            (None,None) => { break; }
        }
    }
    out.make(len)
}

fn intersect(a: &[(usize,usize)], b: &[(usize,usize)],len: usize) -> EachOrEveryFilter {
    let mut a_iter = NumIterator::new(a);
    let mut b_iter = NumIterator::new(b);
    let mut out = EachOrEveryFilterBuilder::new();
    loop {
        match (a_iter.peek(),b_iter.peek()) {
            (Some(a),Some(b)) => {
                if a == b { 
                    out.set(a);
                    a_iter.advance(b+1); 
                    b_iter.advance(a+1);
                } else if a < b { 
                    a_iter.advance(b);
                } else if a > b {
                    b_iter.advance(a);
                }
            },
            _ => { break; }
        }
    }
    out.make(len)
}

#[cfg_attr(debug_assertions,derive(Debug))]
#[derive(Clone)]
enum EachOrEveryFilterData {
    All,
    None,
    Some(Vec<(usize,usize)>)
}

#[cfg_attr(debug_assertions,derive(Debug))]
#[derive(Clone)]
pub struct EachOrEveryFilter {
    data: EachOrEveryFilterData,
    len: usize,
    count: usize
}

impl EachOrEveryFilter {
    pub fn all(len: usize) -> EachOrEveryFilter {
        return EachOrEveryFilter {
            data: EachOrEveryFilterData::All,
            len, count: len
        };
    }

    pub fn none(len: usize) -> EachOrEveryFilter {
        return EachOrEveryFilter {
            data: EachOrEveryFilterData::None,
            len, count: 0
        };
    }

    pub fn len(&self) -> usize { self.len }
    pub fn count(&self) -> usize { self.count }

    pub fn filter_clone<Z: Clone>(&self, input: &[Z]) -> Vec<Z> {
        if input.len() == 0 { return vec![]; }
        match &self.data {
            EachOrEveryFilterData::All => input.to_vec(),
            EachOrEveryFilterData::None => vec![],
            EachOrEveryFilterData::Some(index) => {
                let mut out = vec![];
                for (offset,len) in index {
                    for pos in 0..*len {
                        out.push(input[(offset+pos)%input.len()].clone());
                    }
                }
                out
            }
        }
    }

    pub(super) fn eoe_filter<X>(&self, data: &EachOrEvery<X>) -> EachOrEvery<X> {
        if let Some(len) = data.len() { if self.len() != len {
            panic!("bad filter size self={:?} filter={:?}",len,self.len());
        }}
         match &self.data {
            EachOrEveryFilterData::All => data.clone(),
            EachOrEveryFilterData::None => EachOrEvery::each(vec![]),
            EachOrEveryFilterData::Some(filter) => {
                let index = match &data.index {
                    EachOrEveryIndex::Every => EachOrEveryIndex::Every,
                    EachOrEveryIndex::Unindexed => EachOrEveryIndex::Indexed(un_rle(&filter,|i| i)),
                    EachOrEveryIndex::Indexed(index) => EachOrEveryIndex::Indexed(un_rle(&filter,|i| index[i]))
                };
                EachOrEvery { index, data: data.data.clone() }        
            }
        }
    }

    pub fn and(&self, other: &EachOrEveryFilter) -> EachOrEveryFilter {
        match (&self.data,&other.data) {
            (EachOrEveryFilterData::All,_) => other.clone(),
            (_,EachOrEveryFilterData::All) => self.clone(),
            (EachOrEveryFilterData::None,_) => EachOrEveryFilter::none(self.len),
            (_,EachOrEveryFilterData::None) => EachOrEveryFilter::none(self.len),

            (EachOrEveryFilterData::Some(self_index), EachOrEveryFilterData::Some(other_index)) => {
                intersect(self_index,other_index,self.len)
            }
        }
    }

    pub fn or(&self, other: &EachOrEveryFilter) -> EachOrEveryFilter {
        match (&self.data,&other.data) {
            (EachOrEveryFilterData::All,_) => EachOrEveryFilter::all(self.len()),
            (_,EachOrEveryFilterData::All) => EachOrEveryFilter::all(self.len()),
            (EachOrEveryFilterData::None,_) => other.clone(),
            (_,EachOrEveryFilterData::None) => self.clone(),

            (EachOrEveryFilterData::Some(self_index), EachOrEveryFilterData::Some(other_index)) => {
                union(self_index,other_index,self.len)
            }
        }
    }
}
