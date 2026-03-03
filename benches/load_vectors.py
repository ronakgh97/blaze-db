#!/usr/bin/env python3

import pandas as pd
import struct
import sys
import os
from pathlib import Path


# Dataset Link: https://huggingface.co/datasets/KShivendu/dbpedia-entities-openai-1M
def main():
    downloads = Path(os.path.expanduser("~/Downloads"))
    parquet_files = sorted(downloads.glob("train-*-of-00026-*.parquet"))

    print(f"Found {len(parquet_files)} parquet files", file=sys.stderr)

    max_vectors = 100000
    max_files = 3

    output_dir = Path("./datasets")
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "bench_vectors_100k.bin"

    total = 0
    vectors = []

    for i, path in enumerate(parquet_files[:max_files]):
        print(f"Loading file {i + 1}/{max_files}: {path.name}", file=sys.stderr)
        df = pd.read_parquet(path)

        for val in df["openai"].values:
            vectors.append([float(x) for x in val])
            total += 1

            if total >= max_vectors:
                break

            if total % 10000 == 0:
                print(f"  Processed {total}/{max_vectors} vectors...", file=sys.stderr)

        if total >= max_vectors:
            break

    num_vectors = len(vectors)
    dim = len(vectors[0])

    print(f"Total vectors: {num_vectors}", file=sys.stderr)
    print(f"Vector dimensions: {dim}", file=sys.stderr)

    # Save to binary format: [num_vectors: u32][dim: u32][vectors: f32...]
    with open(output_path, "wb") as f:
        # Write header
        f.write(struct.pack("II", num_vectors, dim))
        # Write vectors
        for vec in vectors:
            f.write(struct.pack(f"{dim}f", *vec))

    print(f"Saved to {output_path}", file=sys.stderr)
    print(
        f"File size: {os.path.getsize(output_path) / 1024 / 1024:.1f} MB",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
