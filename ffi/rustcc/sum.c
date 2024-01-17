#include "sum.h"

Point sum(Point* a, Point* b) {
  Point p = {
    a->x + b->x,
    a->y + b->y
  };
  return p;
}
