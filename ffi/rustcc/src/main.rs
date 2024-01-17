use rustcc::Point;
use rustcc::sum;

fn main() {
    let mut a = Point { x: 10, y: 20 };
    let mut b = Point { x: 30, y: 40 };
    let result = unsafe { sum(&mut a as *mut _, &mut b as *mut _) };

    println!("The sum of {:?} and {:?} is {:?}", a, b, result);
}
