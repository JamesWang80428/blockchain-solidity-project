module M {
    struct S { f: u64, g: u64 }
    id<T>(r: &T): &T {
        r
    }
    id_mut<T>(r: &mut T): &mut T {
        r
    }

    t0() {
        let v = S { f: 0, g: 0 };
        let f = &v.f;
        let s = &v;
        *f;
        *s;
        *f;

        let v = S { f: 0, g: 0 };
        let f = id(&v.f);
        let s = &v;
        *f;
        *s;
        *f;

        let v = S { f: 0, g: 0 };
        let f = &v.f;
        let s = &mut v;
        *f;
        *s;
        *f;
    }

}
