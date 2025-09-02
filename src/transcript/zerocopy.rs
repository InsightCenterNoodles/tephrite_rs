// external enum

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use teph_macro::serde_enum_framework;

    use super::super::{deserialize, TDeserialize, TSerialize};

    struct Test1 {
        a: [i32; 4],
    }
    struct Test2 {
        b: f32,
    }

    impl TSerialize for Test1 {
        fn serialize(&self, w: &mut impl std::io::Write) {
            self.a.serialize(w);
        }
    }
    impl TDeserialize for Test1 {
        fn deserialize(r: &mut impl std::io::Read) -> Self {
            Self { a: deserialize(r) }
        }
    }

    impl TSerialize for Test2 {
        fn serialize(&self, w: &mut impl std::io::Write) {
            self.b.serialize(w);
        }
    }
    impl TDeserialize for Test2 {
        fn deserialize(r: &mut impl std::io::Read) -> Self {
            Self { b: deserialize(r) }
        }
    }

    //

    serde_enum_framework!(TestThing, Test1, Test2,);

    #[derive(Default)]
    struct MyDecoder {
        saw_1: bool,
        saw_2: bool,
    }

    impl DecodeTestThing for MyDecoder {
        fn handle_test1(&mut self, item: Test1) {
            self.saw_1 = true;
            assert_eq!(item.a, [1, 2, 4, 9]);
        }

        fn handle_test2(&mut self, item: Test2) {
            assert!(self.saw_1);
            self.saw_2 = true;
            assert_eq!(item.b, 12.9);
        }
    }

    #[test]
    fn minimal_copy_macros() {
        let mut bytes = Vec::<u8>::new();

        (Test1 { a: [1, 2, 4, 9] }).encode_to(&mut bytes);
        (Test2 { b: 12.9 }).encode_to(&mut bytes);

        let mut cursor = Cursor::new(bytes);

        let mut decoder = MyDecoder::default();

        decode_TestThing(&mut cursor, &mut decoder);
    }
}
