#![expect(
    clippy::missing_docs_in_private_items,
    missing_docs,
    reason = "benchmark harness — no public API"
)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

// ── Types ────────────────────────────────────────────────────────────────────

struct Circle {
    radius: f64,
}
struct Rect {
    w: f64,
    h: f64,
}
struct Triangle {
    a: f64,
    b: f64,
    c: f64,
}
struct Hexagon {
    side: f64,
}

// ── Devirtualized trait (devirt macros) ───────────────────────────────────────

devirt::r#trait! {
    pub Shape [Circle, Rect] {
        fn area(&self) -> f64;
        fn scale(&mut self, factor: f64);
    }
}

devirt::r#impl!(Shape for Circle [hot] {
    fn area(&self) -> f64 {
        core::f64::consts::PI * self.radius * self.radius
    }
    fn scale(&mut self, factor: f64) {
        self.radius *= factor;
    }
});

devirt::r#impl!(Shape for Rect [hot] {
    fn area(&self) -> f64 { self.w * self.h }
    fn scale(&mut self, factor: f64) {
        self.w *= factor;
        self.h *= factor;
    }
});

devirt::r#impl!(Shape for Triangle {
    fn area(&self) -> f64 {
        let s = (self.a + self.b + self.c) / 2.0;
        (s * (s - self.a) * (s - self.b) * (s - self.c)).sqrt()
    }
    fn scale(&mut self, factor: f64) {
        self.a *= factor;
        self.b *= factor;
        self.c *= factor;
    }
});

devirt::r#impl!(Shape for Hexagon {
    fn area(&self) -> f64 { 1.5 * 3.0_f64.sqrt() * self.side * self.side }
    fn scale(&mut self, factor: f64) { self.side *= factor; }
});

// ── Explicit Branch-Based Dispatch ───────────────────────────────────────────
// This shows what pure branch-based dispatch looks like (comparing TypeTag enum)

#[derive(Clone, Copy)]
enum ShapeType {
    Circle,
    Rect,
    Triangle,
    Hexagon,
}

struct BranchShape {
    #[expect(
        dead_code,
        reason = "tag is not read; kept so layout mirrors a typical tag+payload dispatch struct"
    )]
    ty: ShapeType,
    data: BranchShapeData,
}

enum BranchShapeData {
    Circle(Circle),
    Rect(Rect),
    Triangle(Triangle),
    Hexagon(Hexagon),
}

impl BranchShape {
    fn area(&self) -> f64 {
        match &self.data {
            BranchShapeData::Circle(c) => core::f64::consts::PI * c.radius * c.radius,
            BranchShapeData::Rect(r) => r.w * r.h,
            BranchShapeData::Triangle(t) => {
                let s = (t.a + t.b + t.c) / 2.0;
                (s * (s - t.a) * (s - t.b) * (s - t.c)).sqrt()
            }
            BranchShapeData::Hexagon(h) => 1.5 * 3.0_f64.sqrt() * h.side * h.side,
        }
    }

    fn scale(&mut self, factor: f64) {
        match &mut self.data {
            BranchShapeData::Circle(c) => c.radius *= factor,
            BranchShapeData::Rect(r) => {
                r.w *= factor;
                r.h *= factor;
            }
            BranchShapeData::Triangle(t) => {
                t.a *= factor;
                t.b *= factor;
                t.c *= factor;
            }
            BranchShapeData::Hexagon(h) => h.side *= factor,
        }
    }
}

// ── Plain trait (baseline, normal vtable dispatch) ───────────────────────────

trait PlainShape {
    fn area(&self) -> f64;
    fn scale(&mut self, factor: f64);
}

impl PlainShape for Circle {
    fn area(&self) -> f64 {
        core::f64::consts::PI * self.radius * self.radius
    }
    fn scale(&mut self, factor: f64) {
        self.radius *= factor;
    }
}

impl PlainShape for Rect {
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn scale(&mut self, factor: f64) {
        self.w *= factor;
        self.h *= factor;
    }
}

impl PlainShape for Triangle {
    fn area(&self) -> f64 {
        let s = (self.a + self.b + self.c) / 2.0;
        (s * (s - self.a) * (s - self.b) * (s - self.c)).sqrt()
    }
    fn scale(&mut self, factor: f64) {
        self.a *= factor;
        self.b *= factor;
        self.c *= factor;
    }
}

impl PlainShape for Hexagon {
    fn area(&self) -> f64 {
        1.5 * 3.0_f64.sqrt() * self.side * self.side
    }
    fn scale(&mut self, factor: f64) {
        self.side *= factor;
    }
}

// ── Benchmarks ───────────────────────────────────────────────────────────────

fn bench_area(c: &mut Criterion) {
    let mut group = c.benchmark_group("area");

    // devirt hot — Circle is in the hot list
    group.bench_function("devirt_hot", |b| {
        let shape: Box<dyn Shape> = Box::new(Circle { radius: 5.0 });
        let shape_ref: &dyn Shape = &*shape;
        b.iter(|| black_box(shape_ref).area());
    });

    // devirt cold — Triangle falls through to vtable
    group.bench_function("devirt_cold", |b| {
        let shape: Box<dyn Shape> = Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        });
        let shape_ref: &dyn Shape = &*shape;
        b.iter(|| black_box(shape_ref).area());
    });

    // plain hot — same type, normal vtable
    group.bench_function("plain_hot", |b| {
        let shape: Box<dyn PlainShape> = Box::new(Circle { radius: 5.0 });
        let shape_ref: &dyn PlainShape = &*shape;
        b.iter(|| black_box(shape_ref).area());
    });

    // plain cold — same type, normal vtable
    group.bench_function("plain_cold", |b| {
        let shape: Box<dyn PlainShape> = Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        });
        let shape_ref: &dyn PlainShape = &*shape;
        b.iter(|| black_box(shape_ref).area());
    });

    // branch-based hot — explicit enum dispatch (Circle)
    group.bench_function("branch_hot", |b| {
        let shape = BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 5.0 }),
        };
        b.iter(|| black_box(&shape).area());
    });

    // branch-based cold — explicit enum dispatch (Triangle)
    group.bench_function("branch_cold", |b| {
        let shape = BranchShape {
            ty: ShapeType::Triangle,
            data: BranchShapeData::Triangle(Triangle {
                a: 3.0,
                b: 4.0,
                c: 5.0,
            }),
        };
        b.iter(|| black_box(&shape).area());
    });

    group.finish();
}

fn bench_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale");

    group.bench_function("devirt_hot", |b| {
        let mut shape: Box<dyn Shape> = Box::new(Circle { radius: 5.0 });
        b.iter(|| {
            let s: &mut dyn Shape = &mut *shape;
            black_box(s).scale(black_box(2.0));
        });
    });

    group.bench_function("devirt_cold", |b| {
        let mut shape: Box<dyn Shape> = Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        });
        b.iter(|| {
            let s: &mut dyn Shape = &mut *shape;
            black_box(s).scale(black_box(2.0));
        });
    });

    group.bench_function("plain_hot", |b| {
        let mut shape: Box<dyn PlainShape> = Box::new(Circle { radius: 5.0 });
        b.iter(|| {
            let s: &mut dyn PlainShape = &mut *shape;
            black_box(s).scale(black_box(2.0));
        });
    });

    group.bench_function("plain_cold", |b| {
        let mut shape: Box<dyn PlainShape> = Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        });
        b.iter(|| {
            let s: &mut dyn PlainShape = &mut *shape;
            black_box(s).scale(black_box(2.0));
        });
    });

    group.bench_function("branch_hot", |b| {
        let mut shape = BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 5.0 }),
        };
        b.iter(|| {
            black_box(&mut shape).scale(black_box(2.0));
        });
    });

    group.bench_function("branch_cold", |b| {
        let mut shape = BranchShape {
            ty: ShapeType::Triangle,
            data: BranchShapeData::Triangle(Triangle {
                a: 3.0,
                b: 4.0,
                c: 5.0,
            }),
        };
        b.iter(|| {
            black_box(&mut shape).scale(black_box(2.0));
        });
    });

    group.finish();
}

fn make_mixed_shapes_devirt() -> Vec<Box<dyn Shape>> {
    vec![
        Box::new(Circle { radius: 5.0 }),
        Box::new(Rect { w: 3.0, h: 4.0 }),
        Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        }),
        Box::new(Hexagon { side: 2.0 }),
    ]
}

fn make_mixed_shapes_plain() -> Vec<Box<dyn PlainShape>> {
    vec![
        Box::new(Circle { radius: 5.0 }),
        Box::new(Rect { w: 3.0, h: 4.0 }),
        Box::new(Triangle {
            a: 3.0,
            b: 4.0,
            c: 5.0,
        }),
        Box::new(Hexagon { side: 2.0 }),
    ]
}

fn make_mixed_shapes_branch() -> Vec<BranchShape> {
    vec![
        BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 5.0 }),
        },
        BranchShape {
            ty: ShapeType::Rect,
            data: BranchShapeData::Rect(Rect { w: 3.0, h: 4.0 }),
        },
        BranchShape {
            ty: ShapeType::Triangle,
            data: BranchShapeData::Triangle(Triangle {
                a: 3.0,
                b: 4.0,
                c: 5.0,
            }),
        },
        BranchShape {
            ty: ShapeType::Hexagon,
            data: BranchShapeData::Hexagon(Hexagon { side: 2.0 }),
        },
    ]
}

fn make_hot_dominant_shapes_devirt() -> Vec<Box<dyn Shape>> {
    vec![
        Box::new(Circle { radius: 5.0 }),
        Box::new(Circle { radius: 3.0 }),
        Box::new(Circle { radius: 7.0 }),
        Box::new(Circle { radius: 1.0 }),
        Box::new(Rect { w: 3.0, h: 4.0 }),
        Box::new(Rect { w: 5.0, h: 6.0 }),
        Box::new(Rect { w: 2.0, h: 8.0 }),
        Box::new(Rect { w: 1.0, h: 1.0 }),
        Box::new(Triangle { a: 3.0, b: 4.0, c: 5.0 }),
        Box::new(Hexagon { side: 2.0 }),
    ]
}

fn make_hot_dominant_shapes_plain() -> Vec<Box<dyn PlainShape>> {
    vec![
        Box::new(Circle { radius: 5.0 }),
        Box::new(Circle { radius: 3.0 }),
        Box::new(Circle { radius: 7.0 }),
        Box::new(Circle { radius: 1.0 }),
        Box::new(Rect { w: 3.0, h: 4.0 }),
        Box::new(Rect { w: 5.0, h: 6.0 }),
        Box::new(Rect { w: 2.0, h: 8.0 }),
        Box::new(Rect { w: 1.0, h: 1.0 }),
        Box::new(Triangle { a: 3.0, b: 4.0, c: 5.0 }),
        Box::new(Hexagon { side: 2.0 }),
    ]
}

fn make_hot_dominant_shapes_branch() -> Vec<BranchShape> {
    vec![
        BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 5.0 }),
        },
        BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 3.0 }),
        },
        BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 7.0 }),
        },
        BranchShape {
            ty: ShapeType::Circle,
            data: BranchShapeData::Circle(Circle { radius: 1.0 }),
        },
        BranchShape {
            ty: ShapeType::Rect,
            data: BranchShapeData::Rect(Rect { w: 3.0, h: 4.0 }),
        },
        BranchShape {
            ty: ShapeType::Rect,
            data: BranchShapeData::Rect(Rect { w: 5.0, h: 6.0 }),
        },
        BranchShape {
            ty: ShapeType::Rect,
            data: BranchShapeData::Rect(Rect { w: 2.0, h: 8.0 }),
        },
        BranchShape {
            ty: ShapeType::Rect,
            data: BranchShapeData::Rect(Rect { w: 1.0, h: 1.0 }),
        },
        BranchShape {
            ty: ShapeType::Triangle,
            data: BranchShapeData::Triangle(Triangle {
                a: 3.0,
                b: 4.0,
                c: 5.0,
            }),
        },
        BranchShape {
            ty: ShapeType::Hexagon,
            data: BranchShapeData::Hexagon(Hexagon { side: 2.0 }),
        },
    ]
}

fn bench_mixed_vec(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_vec");

    group.bench_function("devirt", |b| {
        let shapes = make_mixed_shapes_devirt();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s.as_ref()).area();
            }
            total
        });
    });

    group.bench_function("plain", |b| {
        let shapes = make_mixed_shapes_plain();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s.as_ref()).area();
            }
            total
        });
    });

    group.bench_function("branch", |b| {
        let shapes = make_mixed_shapes_branch();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s).area();
            }
            total
        });
    });

    group.bench_function("devirt_hot_dominant", |b| {
        let shapes = make_hot_dominant_shapes_devirt();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s.as_ref()).area();
            }
            total
        });
    });

    group.bench_function("plain_hot_dominant", |b| {
        let shapes = make_hot_dominant_shapes_plain();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s.as_ref()).area();
            }
            total
        });
    });

    group.bench_function("branch_hot_dominant", |b| {
        let shapes = make_hot_dominant_shapes_branch();
        b.iter(|| {
            let mut total = 0.0_f64;
            for s in &shapes {
                total += black_box(s).area();
            }
            total
        });
    });

    group.finish();
}

criterion_group!(benches, bench_area, bench_scale, bench_mixed_vec);
criterion_main!(benches);
