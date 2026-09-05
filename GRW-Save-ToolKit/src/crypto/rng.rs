#[derive(Debug, Clone)]
pub struct GrwRng {
    ma: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl GrwRng {
    pub fn new(seed: u32) -> Self {
        let mut num = seed as i64;
        if num > 0 {
            num = -num;
        }
        let mut num2 = ((161803398_i64 - num.abs()) % 10000000) as i32;
        if num2 < 0 {
            num2 += 10000000;
        }

        let mut ma = [0_i32; 56];
        ma[55] = num2;
        let mut num3 = 1_i32;

        for i in 1..55 {
            let num4 = (21 * i) % 55;
            ma[num4] = num3;
            num3 = num2 - num3;
            if num3 < 0 {
                num3 += 10000000;
            }
            num2 = ma[num4];
        }

        for _ in 0..4 {
            for k in 1..56 {
                ma[k] -= ma[1 + (k + 30) % 55];
                if ma[k] < 0 {
                    ma[k] += 10000000;
                }
            }
        }

        Self {
            ma,
            inext: 0,
            inextp: 31,
        }
    }

    pub fn next(&mut self) -> i32 {
        self.inext += 1;
        if self.inext == 56 {
            self.inext = 1;
        }
        self.inextp += 1;
        if self.inextp == 56 {
            self.inextp = 1;
        }

        let mut num = self.ma[self.inext] - self.ma[self.inextp];
        if num < 0 {
            num += 10000000;
        }
        self.ma[self.inext] = num;
        num
    }
}
