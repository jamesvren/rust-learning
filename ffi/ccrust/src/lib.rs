#[repr(C)]
pub struct Point {
    x: i64,
    y: i64,
}

#[no_mangle]
pub extern "C" fn add(left: Point, right: Point) -> Point {
    Point {
        x: left.x + right.x,
        y: left.y + right.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(Point {x: 2, y: 2}, Point {x:2, y:2});
        assert_eq!(result, Point {x:4, y:4});
    }
}
