use crate::decode::Decode;
use crate::encode::Encode;
use crate::types::Type;
use crate::postgres::protocol::TypeId;
use crate::postgres::{ PgData, PgValue, PgTypeInfo, Postgres };
use crate::io::Buf;
use geo::Coordinate;
use byteorder::BigEndian;

// <https://www.postgresql.org/docs/12/datatype-geometric.html>

impl Type<Postgres> for Coordinate<f64> {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::new(TypeId::POINT, "POINT")
    }
}

impl<'de> Decode<'de, Postgres> for Coordinate<f64> {
    fn decode(value: PgValue<'de>) -> crate::Result<Self> {
        match value.try_get()? {
            PgData::Binary(mut buf) => {
                // this should be a (
                let open_paren = buf.get_str(1)?;
                println!("starting with {}", open_paren);
                
                let x = buf.get_f64::<BigEndian>()?;
                println!("then we have what is hopefully x: {}", x);
                
                // this should be a ,
                let comma = buf.get_str(1)?;
                println!("pause with a comma! {}", comma);
                
                let y = buf.get_f64::<BigEndian>()?;
                println!("is this a y? {}", y);
                
                // this should be a )
                let close_paren = buf.get_str(1)?;
                println!("let's finish strong with a {}", close_paren);

                Ok((x, y).into())
            }

            PgData::Text(s) => {
                unimplemented!()
            }
        }
    }
}

// #[test]
// fn test_decode_coordinate() {
//     // (5.0, 45.5)
//     let mut bytes = [0; 19];
//     bytes.put_u8("(".as_bytes());
//     bytes.put_f64(5.0);
//     bytes.put_u8(",".as_bytes());
//     bytes.put_f64(45.5);
//     bytes.put_u8("(".as_bytes());
//     let point = Decode::<Postgres>::decode(PgValue::from_bytes(&bytes)).unwrap();
//     assert_eq!(point, Coordinate::from((5.0, 45.5)));
// }