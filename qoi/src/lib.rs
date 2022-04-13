pub fn xrgb8888_encode_qoi(bytes: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    const OP_RGB: u8 = 0b1111_1110;
    const OP_INDEX: u8 = 0b0000_0000;
    const OP_DIFF: u8 = 0b0100_0000;
    const OP_LUMA: u8 = 0b1000_0000;
    const OP_RUN: u8 = 0b1100_0000;

    let mut res = vec![];
    let width_bytes_be = width.to_be_bytes();
    let height_bytes_be = height.to_be_bytes();
    let header = [
        b'q',
        b'o',
        b'i',
        b'f',
        width_bytes_be[0],
        width_bytes_be[1],
        width_bytes_be[2],
        width_bytes_be[3],
        height_bytes_be[0],
        height_bytes_be[1],
        height_bytes_be[2],
        height_bytes_be[3],
        3,
        0,
    ];
    res.extend_from_slice(&header);
    let mut prev_pixel = [0, 0, 0, 0xff];
    let mut array = [[0; 4]; 64];
    let mut run_length = 0;
    for line in bytes.chunks_exact(stride as _) {
        for &pixel in array_chunks::<_, 4>(&line[..(width * 4) as _]) {
            let pixel = [pixel[2], pixel[1], pixel[0], 0xff];
            if pixel == prev_pixel {
                run_length += 1;
                if run_length == 62 {
                    res.push(OP_RUN | (run_length - 1));
                    run_length = 0;
                }
                continue;
            }
            if run_length > 0 {
                res.push(OP_RUN | (run_length - 1));
                run_length = 0;
            }
            let prev = prev_pixel;
            prev_pixel = pixel;
            let index = {
                let sum = 0u8
                    .wrapping_add(pixel[0].wrapping_mul(3))
                    .wrapping_add(pixel[1].wrapping_mul(5))
                    .wrapping_add(pixel[2].wrapping_mul(7))
                    .wrapping_add(255u8.wrapping_mul(11));
                sum & 63
            };
            if array[index as usize] == pixel {
                res.push(OP_INDEX | index);
                continue;
            }
            array[index as usize] = pixel;
            let dr = pixel[0].wrapping_sub(prev[0]);
            let dg = pixel[1].wrapping_sub(prev[1]);
            let db = pixel[2].wrapping_sub(prev[2]);
            let dr_2 = dr.wrapping_add(2);
            let dg_2 = dg.wrapping_add(2);
            let db_2 = db.wrapping_add(2);
            if dr_2 | dg_2 | db_2 | 3 == 3 {
                res.push(OP_DIFF | (dr_2 << 4) | (dg_2 << 2) | db_2);
                continue;
            }
            let dr_dg_8 = dr.wrapping_sub(dg).wrapping_add(8);
            let db_dg_8 = db.wrapping_sub(dg).wrapping_add(8);
            let dg_32 = dg.wrapping_add(32);
            if (dg_32 | 63 == 63) && (dr_dg_8 | db_dg_8 | 15 == 15) {
                res.extend_from_slice(&[OP_LUMA | dg_32, (dr_dg_8 << 4) | db_dg_8]);
                continue;
            }
            res.extend_from_slice(&[OP_RGB, pixel[0], pixel[1], pixel[2]]);
        }
    }
    if run_length > 0 {
        res.push(OP_RUN | (run_length - 1));
    }
    res.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    res
}

fn array_chunks<T, const N: usize>(slice: &[T]) -> &[[T; N]] {
    let len = slice.len() / N;
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as _, len) }
}
