// Time series type. Measures seconds since UNIX epoch.

/// A time-series type for storing a consecutive set of temporal data.
#[derive(Debug, Clone)]
pub struct TimeSeries<T> {
    pub timestamps: Vec<T>,
}

/// Implementations for TimeSeries
impl<T> TimeSeries<T> {
    /// Initialize a TimeSeries from a vector of timestamps
    pub fn from_vec(timestamps: Vec<T>) -> Self {
        TimeSeries { timestamps }
    }

    /// Initialize a TimeSeries with a given length
    pub fn from_len(len: usize) -> Self {
        TimeSeries {
            timestamps: Vec::with_capacity(len),
        }
    }

    /// Initialize a TimeSeries from a sparse map of timestamps
    pub fn from_sparse_map(
        sparse_map: &std::collections::HashMap<i64, T>,
        len: usize,
        default: T,
    ) -> Self
    where
        T: Clone,
    {
        let mut timestamps = vec![default; len];
        for (index, (_, value)) in sparse_map.iter().enumerate() {
            if index < len {
                timestamps[index] = value.clone();
            }
        }
        TimeSeries { timestamps }
    }

    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.timestamps.get(index)
    }

    pub fn set(&mut self, index: usize, value: T) {
        if index < self.timestamps.len() {
            self.timestamps[index] = value;
        }
    }

    pub fn push(&mut self, value: T) {
        self.timestamps.push(value);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.timestamps.iter()
    }

    pub fn slice(&self, range: std::ops::Range<usize>) -> Self
    where
        T: Clone,
    {
        TimeSeries {
            timestamps: self.timestamps[range].to_vec(),
        }
    }
}
