use slotmap::{DefaultKey, SlotMap};
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
};

struct FieldMap2<Owner, Field1, Field2, Getter1, Getter2> {
    data: SlotMap<DefaultKey, Owner>,
    map1: RefCell<InnerFieldMap<Owner, Field1, Getter1>>,
    map2: RefCell<InnerFieldMap<Owner, Field2, Getter2>>,
}

impl<Owner, Field1, Field2, Getter1, Getter2> FieldMap2<Owner, Field1, Field2, Getter1, Getter2>
where
    Field1: Hash + Eq,
    Field2: Hash + Eq,
    Getter1: Fn(&Owner) -> &Field1,
    Getter2: Fn(&Owner) -> &Field2,
{
    pub fn new(k1g: Getter1, k2g: Getter2) -> Self {
        Self {
            data: SlotMap::new(),
            map1: RefCell::new(InnerFieldMap::new(k1g)),
            map2: RefCell::new(InnerFieldMap::new(k2g)),
        }
    }

    pub fn from_iter<I>(
        k1g: Getter1,
        k2g: Getter2,
        iter: I,
    ) -> Result<Self, FieldOverlapError<'static, Owner>>
    where
        I: IntoIterator<Item = Owner>,
    {
        let mut map = Self::new(k1g, k2g);
        map.insert_multi(iter).map_err(|err| err.make_static())?;
        Ok(map)
    }

    pub fn get_k1(&self, k1: &Field1) -> Option<&Owner> {
        // SAFETY: self.insert() ensures map validity
        let slotkey = unsafe { self.map1.borrow_mut().get(k1, &self.data)? };
        self.data.get(slotkey)
    }

    pub fn get_k2(&self, k2: &Field2) -> Option<&Owner> {
        // SAFETY: self.insert() ensures map validity
        let slotkey = unsafe { self.map2.borrow_mut().get(k2, &self.data)? };
        self.data.get(slotkey)
    }

    pub fn insert_multi<I>(&mut self, iter: I) -> Result<(), FieldOverlapError<Owner>>
    where
        I: IntoIterator<Item = Owner>,
    {
        // TODO: specialize insert_multi to only invalidate the rebuild the
        // inner maps once at the end.

        // The following crappy code is equivalent to:
        //
        // for owner in iter {
        //     self.insert(owner)?;
        // }
        //
        // but the above does not compile due to borrow checker
        // limitations. The above DOES compile using -Zpolonius,
        // and it is obviously sound anyway.

        let mut err_data: Option<(*const Owner, Owner)> = None;

        for owner in iter {
            if let Err(e) = self.insert(owner) {
                err_data = Some((e.existing.unwrap(), e.new));
                break;
            }
        }

        if let Some((existing, new)) = err_data {
            Err(FieldOverlapError {
                existing: unsafe { existing.as_ref() },
                new,
            })
        } else {
            Ok(())
        }
    }

    pub fn insert(&mut self, owner: Owner) -> Result<(), FieldOverlapError<Owner>> {
        let map1 = self.map1.get_mut();
        let map2 = self.map2.get_mut();
        let overlap_1 = unsafe { map1.contains(&owner, &self.data) };
        let overlap_2 = unsafe { map2.contains(&owner, &self.data) };

        if let Some(key) = overlap_1.or(overlap_2) {
            let overlap = &self.data[key];
            return Err(FieldOverlapError {
                existing: Some(overlap),
                new: owner,
            });
        }

        // If self.data.capacity() == self.data.len(), then
        // self.data will reallocate upon insert. The maps must be
        // invalidated, because their UnsafePtrKeys will become dangling.
        let invalidate = self.data.capacity() == self.data.len();
        let key = self.data.insert(owner);
        if invalidate {
            map1.invalidate();
            map2.invalidate();
        } else {
            // An insert is only necessary when the field maps remain valid,
            // because invalidation causes a rebuild, and a rebuild
            // automatically inserts all keys from self.data anyway.
            let owner = &self.data[key];
            map1.insert(owner, key);
            map2.insert(owner, key);
        }

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Owner> {
        self.data.iter().map(|(_, owner)| owner)
    }
}

struct InnerFieldMap<Owner, Field, Getter> {
    valid: bool,
    map: HashMap<UnsafePtrKey<Field>, DefaultKey>,
    getter: Getter,
    _phantom_getter: PhantomData<fn(&Owner) -> &Field>,
}

impl<Owner, Field, Getter> InnerFieldMap<Owner, Field, Getter>
where
    Field: Hash + Eq,
    Getter: Fn(&Owner) -> &Field,
{
    fn new(getter: Getter) -> Self {
        Self {
            valid: false,
            map: HashMap::new(),
            getter,
            _phantom_getter: PhantomData,
        }
    }

    fn invalidate(&mut self) {
        self.valid = false;
    }

    // SAFETY: The self.valid flag must be accurate. If the Owners of Fields may
    // have moved, i.e. because the owning SlotMap has been mutated, this map
    // must be marked invalid before calling get().
    unsafe fn get<'a, I>(&mut self, k: &Field, iter: I) -> Option<DefaultKey>
    where
        Owner: 'a,
        I: IntoIterator<Item = (DefaultKey, &'a Owner)>,
    {
        if !self.valid {
            self.rebuild(iter);
        }

        self.map.get(&UnsafePtrKey(k)).copied()
    }

    // SAFETY: See Self::get.
    unsafe fn contains<'a, I>(&mut self, owner: &Owner, iter: I) -> Option<DefaultKey>
    where
        Owner: 'a,
        I: IntoIterator<Item = (DefaultKey, &'a Owner)>,
    {
        let k = (self.getter)(owner);
        self.get(k, iter)
    }

    fn rebuild<'a, I>(&mut self, iter: I)
    where
        Owner: 'a,
        I: IntoIterator<Item = (DefaultKey, &'a Owner)>,
    {
        self.map.clear();
        self.valid = true;

        for (key, owner) in iter {
            self.insert(owner, key);
        }
    }

    fn insert(&mut self, owner: &Owner, key: DefaultKey) {
        let field = (self.getter)(owner);
        self.map.insert(UnsafePtrKey(field), key);
    }
}

struct UnsafePtrKey<T>(*const T);

impl<T: PartialEq> PartialEq for UnsafePtrKey<T> {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: It is a precondition of constructing this struct that self.0 is valid
        unsafe { (*self.0).eq(&*other.0) }
    }
}

impl<T: Eq> Eq for UnsafePtrKey<T> {}

impl<T: Hash> Hash for UnsafePtrKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // SAFETY: It is a precondition of constructing this struct that self.0 is valid
        unsafe { (*self.0).hash(state) }
    }
}

#[derive(Debug)]
struct FieldOverlapError<'a, Owner> {
    existing: Option<&'a Owner>,
    new: Owner,
}

impl<'a, Owner> FieldOverlapError<'a, Owner> {
    fn make_static(self) -> FieldOverlapError<'static, Owner> {
        FieldOverlapError {
            existing: None,
            new: self.new,
        }
    }
}

impl<'a, Owner> Display for FieldOverlapError<'a, Owner>
where
    Owner: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FieldMap insert caused overlapping fields: {:?} overlaps with ",
            self.new
        )?;

        match self.existing {
            Some(x) => write!(f, "{x:?}"),
            None => write!(f, "a prior value"),
        }
    }
}

impl<'a, Owner> Error for FieldOverlapError<'a, Owner> where Owner: Debug {}

#[test]
fn fieldmap2() {
    #[derive(Debug, PartialEq, Eq)]
    struct S(i32, i32, i32);

    impl S {
        fn a(&self) -> &i32 {
            &self.0
        }

        fn b(&self) -> &i32 {
            &self.1
        }
    }

    // let mut map: FieldMap2<S, i32, i32, _, _> =
    //     FieldMap2::from_iter(S::a, S::b, [S(1, 2, 3), S(4, 5, 6), S(7, 8, 9)]).unwrap();
    let mut map: FieldMap2<S, i32, i32, _, _> = FieldMap2::new(S::a, S::b);

    map.insert(S(1, 2, 3)).unwrap();
    assert_eq!(map.get_k1(&1), Some(&S(1, 2, 3)));
    assert_eq!(map.get_k2(&2), Some(&S(1, 2, 3)));

    map.insert(S(4, 5, 6)).unwrap();
    assert_eq!(map.get_k1(&1), Some(&S(1, 2, 3)));
    assert_eq!(map.get_k2(&2), Some(&S(1, 2, 3)));
    assert_eq!(map.get_k1(&4), Some(&S(4, 5, 6)));
    assert_eq!(map.get_k2(&5), Some(&S(4, 5, 6)));

    map.insert(S(7, 8, 9)).unwrap();
    assert_eq!(map.get_k1(&1), Some(&S(1, 2, 3)));
    assert_eq!(map.get_k2(&2), Some(&S(1, 2, 3)));
    assert_eq!(map.get_k1(&4), Some(&S(4, 5, 6)));
    assert_eq!(map.get_k2(&5), Some(&S(4, 5, 6)));
    assert_eq!(map.get_k1(&7), Some(&S(7, 8, 9)));
    assert_eq!(map.get_k2(&8), Some(&S(7, 8, 9)));

    assert!(map.get_k1(&0).is_none());
    assert!(map.get_k2(&0).is_none());

    assert!(map.insert(S(10, 11, 12)).is_ok());
    assert!(map.insert(S(13, 14, 15)).is_ok());

    assert!(map.insert(S(10, -1, -1)).is_err());
    assert!(map.insert(S(-1, 11, -1)).is_err());
    assert!(map.insert(S(-1, -1, 12)).is_ok());

    assert!(map
        .insert_multi([S(20, 30, 40), S(50, 60, 70), S(80, 90, 100)])
        .is_ok());
}
