use core::panic;
use hashbrown::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::eoefilter::{EachOrEveryFilterBuilder, EachOrEveryFilter};

#[cfg_attr(debug_assertions,derive(Debug))]
#[derive(Clone,Hash)]
pub(super) enum EachOrEveryIndex {
    Unindexed,
    Indexed(Arc<Vec<usize>>),
    Every
}

#[cfg_attr(debug_assertions,derive(Debug))]
pub struct EachOrEvery<X> {
    pub(super) index: EachOrEveryIndex,
    pub(super) data: Arc<Vec<X>>
}

impl<X> Clone for EachOrEvery<X> {
    fn clone(&self) -> Self {
        Self { index: self.index.clone(), data: self.data.clone() }
    }
}

impl<X: Hash> Hash for EachOrEvery<X> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.data.hash(state);
    }
}

impl<X> EachOrEvery<X> {
    pub fn each(data: Vec<X>) -> EachOrEvery<X> {
        EachOrEvery {
            index: EachOrEveryIndex::Unindexed,
            data: Arc::new(data)
        }
    }

    pub fn every(data: X) -> EachOrEvery<X> {
        EachOrEvery {
            index: EachOrEveryIndex::Every,
            data: Arc::new(vec![data])
        }
    }

    pub fn len(&self) -> Option<usize> {
        match &self.index {
            EachOrEveryIndex::Unindexed => Some(self.data.len()),
            EachOrEveryIndex::Indexed(index) => Some(index.len()),
            EachOrEveryIndex::Every => None
        }
    }

    pub fn space(&self) -> usize { self.data.len() }

    pub fn get(&self, pos: usize) -> Option<&X> {
        match &self.index {
            EachOrEveryIndex::Unindexed => self.data.get(pos),
            EachOrEveryIndex::Indexed(index) => self.data.get(index[pos]),
            EachOrEveryIndex::Every => self.data.get(0)
        }
    }

    fn unsquash<F,K: Clone+Hash+Eq>(&self, cb: F) -> (Vec<(K,EachOrEveryFilterBuilder)>,Vec<usize>) where F: Fn(&X) -> K {
        /* Optimised hot-path: main objsective is to minimise operations done per index,
         * iterating as much as we can only over data instead. This will be much smaller
         * for indexed values.
         */
        let mut builders = vec![];
        let mut key_to_builder = HashMap::new();
        let mut builder_choices = vec![];
        for key in self.data.iter().map(cb) {
            if let Some(choice) = key_to_builder.get(&key) {
                builder_choices.push(*choice);
            } else {
                builder_choices.push(builders.len());
                key_to_builder.insert(key.clone(),builders.len());
                builders.push((key.clone(),EachOrEveryFilterBuilder::new()));
            }
        }
        (builders,builder_choices)
    }

    pub fn demerge<F,K: Clone+Hash+Eq>(&self, len: usize, cb: F) -> Vec<(K,EachOrEveryFilter)> where F: Fn(&X) -> K {
        match &self.index {
            EachOrEveryIndex::Unindexed => {
                let (mut out,mapped_dest) = self.unsquash(cb);
                for (i,value) in mapped_dest.iter().enumerate() {
                    out[mapped_dest[*value]].1.set(i);
                }
                out.drain(..).map(|(key,filter)| (key,filter.make(len))).collect::<Vec<_>>()
            },
            EachOrEveryIndex::Indexed(index) => {
                let (mut out,mapped_dest) = self.unsquash(cb);
                for (i,value) in index.iter().enumerate() {
                    out[mapped_dest[*value]].1.set(i);
                }
                out.drain(..).map(|(key,filter)| (key,filter.make(len))).collect::<Vec<_>>()
            },
            EachOrEveryIndex::Every => vec![(cb(&self.data[0]),EachOrEveryFilter::all(len))]
        }
    }

    pub fn map<F,Y>(&self, mut f: F) -> EachOrEvery<Y> where F: FnMut(&X) -> Y {
        /* not using functional style because code path is hot */
        let mut new_data = Vec::with_capacity(self.data.len());
        for e in self.data.iter() {
            new_data.push(f(e));
        }
        EachOrEvery {
            index: self.index.clone(),
            data: Arc::new(new_data)
        }
    }
    
    pub fn map_mut<F>(&mut self, f: F) where F: Fn(&X) -> X {
        self.data = Arc::new(self.data.iter().map(f).collect::<Vec<_>>());
    }

    pub fn fold_mut<F,Z>(&mut self, data: &[Z], f: F) where F: Fn(&X,&Z) -> X {
        match &self.index {
            EachOrEveryIndex::Every | EachOrEveryIndex::Unindexed => {
                self.data = Arc::new(self.data.iter().zip(data.iter().cycle()).map(|(x,z)| f(x,z)).collect::<Vec<_>>());
            },
            EachOrEveryIndex::Indexed(index) => {
                let mut out = vec![];
                for (i,z) in index.iter().zip(data.iter()) {
                    out.push(f(&self.data[*i],z));
                }
                self.data = Arc::new(out);
                self.index = EachOrEveryIndex::Unindexed;
            }
        }
    }

    pub fn map_results<F,Y,E>(&self, f: F) -> Result<EachOrEvery<Y>,E> where F: FnMut(&X) -> Result<Y,E> {
        let data = self.data.iter().map(f).collect::<Result<_,_>>()?;
        Ok(EachOrEvery {
            index: self.index.clone(),
            data: Arc::new(data)
        })
    }

    fn inner_zip<W,F,Y>(&self, other: &EachOrEvery<Y>, cb: F) -> EachOrEvery<W> where F: Fn(&X,&Y) -> W {
        match (&self.index,&other.index) {
            (x,EachOrEveryIndex::Every) => {
                EachOrEvery {
                    index: x.clone(),
                    data: Arc::new(self.data.iter().map(|a| cb(a,&other.data[0])).collect())
                }
            },

            (EachOrEveryIndex::Unindexed, EachOrEveryIndex::Unindexed) => {
                EachOrEvery {
                    index: EachOrEveryIndex::Unindexed,
                    data: Arc::new(self.data.iter().zip(other.data.iter()).map(|(a,b)| cb(a,b)).collect())
                }
            },

            (EachOrEveryIndex::Indexed(index), EachOrEveryIndex::Unindexed) => {
                EachOrEvery {
                    index: EachOrEveryIndex::Unindexed,
                    data: Arc::new(index.iter().zip(other.data.iter()).map(|(a,b)| cb(&self.data[*a],b)).collect())
                }
            },

            (EachOrEveryIndex::Indexed(self_index), EachOrEveryIndex::Indexed(other_index)) => {
                EachOrEvery {
                    index: EachOrEveryIndex::Unindexed,
                    data: Arc::new(self_index.iter().zip(other_index.iter()).map(|(a,b)| cb(&self.data[*a],&other.data[*b])).collect())
                }
            },

            _ => panic!()
        }
    }

    pub fn zip<W,F,Y>(&self, other: &EachOrEvery<Y>, cb: F) -> EachOrEvery<W> where F: Fn(&X,&Y) -> W {
        match (&self.index,&other.index) {
            (EachOrEveryIndex::Every, EachOrEveryIndex::Unindexed) |
            (EachOrEveryIndex::Every, EachOrEveryIndex::Indexed(_)) |
            (EachOrEveryIndex::Unindexed, EachOrEveryIndex::Indexed(_)) => 
                other.inner_zip(self,|a,b| cb(b,a)),

            _ =>
                self.inner_zip(other,cb)
        }
    }

    pub fn iter<'a>(&'a self, len: usize) -> Option<impl Iterator<Item=&'a X>> {
        if let Some(self_len) = self.len() {
            if self_len != len { return None; }
        }
        Some(EachOrEveryIterator {
            obj: self,
            index: 0,
            len
        })
    }

    pub fn make_filter<F>(&self, len: usize, cb: F) -> EachOrEveryFilter where F: Fn(&X) -> bool {
        match &self.index {
            EachOrEveryIndex::Unindexed => {
                let mut filter = EachOrEveryFilterBuilder::new();
                for (i,value) in self.data.iter().enumerate() {
                    if cb(value) {
                        filter.set(i);
                    }
                }
                filter.make(len)
            },
            EachOrEveryIndex::Indexed(index) => {
                let mut filter = EachOrEveryFilterBuilder::new();
                for (i,value) in index.iter().enumerate() {
                    if cb(&self.data[*value]) {
                        filter.set(i);
                    }
                }
                filter.make(len)
            },
            EachOrEveryIndex::Every => {
                if cb(&self.data[0]) {
                    EachOrEveryFilter::all(len)
                } else {
                    EachOrEveryFilter::none(len)
                }
            }
        }
    }

    pub fn filter(&self, data_filter: &EachOrEveryFilter) -> EachOrEvery<X> {
        data_filter.eoe_filter(self)
    }

    pub fn to_each(&self, len: usize) -> Option<EachOrEvery<X>> {
        match &self.index {
            EachOrEveryIndex::Every => {
                Some(EachOrEvery {
                    index: EachOrEveryIndex::Indexed(Arc::new(vec![0;len])),
                    data: self.data.clone()
                })
            },
            EachOrEveryIndex::Unindexed => {
                if self.data.len() == len { Some(self.clone()) } else { None }
            },
            EachOrEveryIndex::Indexed(index) => {
                if index.len() == len { Some(self.clone()) } else { None }
            }
        }
    }

    pub fn compatible(&self, len: usize) -> bool {
        if let Some(self_len) = self.len() {
            if self_len != len { return false; }
        }
        true
    }
}

impl<X: Clone> EachOrEvery<X> {
    /* For the data array of this EachOrEvery, merge equivalent values and return a list
     * of indexes and data. This can be used directly by unindexed EoEs. For Indexed EoEs,
     * the "index" returned maps *old* index values to *new* ones.
     */
    fn squash<F,Z>(&self, cb: F) -> (Vec<usize>,Vec<X>) where F: Fn(&X) -> Z, Z: Eq+Hash {
        let mut index = vec![];
        let mut data = vec![];
        let mut map = HashMap::new();
        for item in self.data.iter() {
            let x = cb(item);
            if let Some(pos) = map.get(&x).copied() {
                index.push(pos);
            } else {
                index.push(data.len());
                map.insert(x,data.len());
                data.push(item.clone());
            }
        }
        (index,data)
    }

    pub fn index<F,Z>(&self, cb: F) -> EachOrEvery<X> where F: Fn(&X) -> Z, Z: Eq+Hash {
        match &self.index {
            EachOrEveryIndex::Unindexed => {
                let (index,data) = self.squash(cb);
                EachOrEvery {
                    index: EachOrEveryIndex::Indexed(Arc::new(index)),
                    data: Arc::new(data)
                }
            },
            EachOrEveryIndex::Indexed(old_index) => {
                let (old_to_new,data) = self.squash(cb);
                let index : Vec<_> = old_index.iter().map(|old| old_to_new[*old]).collect();
                EachOrEvery {
                    index: EachOrEveryIndex::Indexed(Arc::new(index)),
                    data: Arc::new(data)
                }
            },
            EachOrEveryIndex::Every => { self.clone() }
        }
    }
}

pub struct EachOrEveryIterator<'a,X> {
    obj: &'a EachOrEvery<X>,
    index: usize,
    len: usize
}

impl<'a,X> Iterator for EachOrEveryIterator<'a,X> {
    type Item = &'a X;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.len { return None; }
        let out = match &self.obj.index {
            EachOrEveryIndex::Unindexed => &self.obj.data[self.index],
            EachOrEveryIndex::Indexed(index) => &self.obj.data[index[self.index]],
            EachOrEveryIndex::Every => &self.obj.data[0]
        };
        self.index += 1;
        Some(out)
    }
}

#[derive(Clone)]
pub enum EachOrEveryGroupCompatible {
    Any,
    Require(usize),
    Invalid
}

impl EachOrEveryGroupCompatible {
    pub fn new(len: Option<usize>) -> EachOrEveryGroupCompatible {
        if let Some(len) = len { EachOrEveryGroupCompatible::Require(len) } else { EachOrEveryGroupCompatible::Any }
    }

    pub fn add<T: Clone>(&mut self, item: &EachOrEvery<T>) -> EachOrEveryGroupCompatible {
        *self = match (self.clone(),item.len()) {
            (EachOrEveryGroupCompatible::Any,Some(len)) => EachOrEveryGroupCompatible::Require(len),
            (EachOrEveryGroupCompatible::Require(len2),Some(len)) if len != len2 => {
                EachOrEveryGroupCompatible::Invalid
            },
            (x,_) => x.clone()
        };
        self.clone()
    }

    pub fn len(&self) -> Option<usize> {
        match self {
            EachOrEveryGroupCompatible::Require(x) => Some(*x),
            _ => None
        }
    }

    pub fn compatible(&self) -> bool {
        match self {
            EachOrEveryGroupCompatible::Invalid => false,
            _ => true
        }
    }

    pub fn complete(&self) -> bool {
        self.len().is_some()
    }
}
