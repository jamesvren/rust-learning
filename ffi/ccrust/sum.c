#include <stdio.h>
#include "sum.h"

void main() {
    Point a = { 2, 2 };
    Point b = { 2, 2 };

    Point c = add(a, b);

    printf("result: x=%ld, y=%ld\n", c.x, c.y);
}
