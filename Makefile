# Прості команди для керування проектом
all: build

build:
	cargo build --release

run:
	cargo run --release

clean:
	cargo clean
	rm -f quadrisrah_map.png

.PHONY: all build run clean