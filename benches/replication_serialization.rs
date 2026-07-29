use bevy::prelude::*;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tephrite_rs::serialize::{ByteWriter, FastWrite};

const BUFFER_SIZE: usize = 16 * 1024 * 1024;

fn write_transforms(transforms: &[Transform], buffer: &mut [u8]) -> usize {
    let mut writer = ByteWriter::new(buffer);
    for transform in transforms {
        unsafe { transform.write_fast(&mut writer) };
    }
    writer.position()
}

fn make_transforms(count: usize) -> Vec<Transform> {
    (0..count)
        .map(|i| {
            let f = i as f32;
            Transform::from_xyz(f * 0.01, f.sin(), f.cos())
                .with_rotation(Quat::from_rotation_y(f * 0.001))
                .with_scale(Vec3::splat(1.0 + (f % 17.0) * 0.001))
        })
        .collect()
}

fn replication_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("replication_serialization");

    for count in [1_000, 10_000, 100_000] {
        let transforms = make_transforms(count);
        group.bench_function(format!("transform_payloads_{count}"), |b| {
            b.iter_batched_ref(
                || vec![0u8; BUFFER_SIZE],
                |buffer| black_box(write_transforms(&transforms, buffer)),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(benches, replication_serialization);
criterion_main!(benches);
