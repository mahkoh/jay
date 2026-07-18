pub fn triangle_contains_points(triangle: [[f64; 2]; 3], points: [[f64; 2]; 3]) -> bool {
    let triangle_vectors = [
        [
            triangle[1][0] - triangle[0][0],
            triangle[1][1] - triangle[0][1],
        ],
        [
            triangle[2][0] - triangle[1][0],
            triangle[2][1] - triangle[1][1],
        ],
        [
            triangle[0][0] - triangle[2][0],
            triangle[0][1] - triangle[2][1],
        ],
    ];
    let mut class = [0u64; 3];
    for i in 0..3 {
        let point = points[i];
        let point_vectors = [
            [point[0] - triangle[0][0], point[1] - triangle[0][1]],
            [point[0] - triangle[1][0], point[1] - triangle[1][1]],
            [point[0] - triangle[2][0], point[1] - triangle[2][1]],
        ];
        for j in 0..3 {
            let triangle_vector = triangle_vectors[j];
            let point_vector = point_vectors[j];
            let cross_product =
                triangle_vector[0] * point_vector[1] - triangle_vector[1] * point_vector[0];
            if cross_product != 0.0 {
                // 0b01 = positive
                // 0b10 = negative
                // 0b11 = mixed positive and negative
                class[i] |= (cross_product.to_bits() >> 63) + 1;
            }
        }
    }
    let mut bad = false;
    for i in 0..3 {
        bad |= class[i] == 0b11;
    }
    !bad
}
