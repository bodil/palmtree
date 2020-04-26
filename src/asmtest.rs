use sized_chunks::Chunk;

pub fn asmtest(a: usize, b: usize) -> Chunk<usize> {
    let mut chunk = Chunk::new();
    chunk.insert_from(0, vec![a, b]);
    chunk
}
