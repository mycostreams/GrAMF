use serde::{Deserialize, Serialize};
// Time series type. Measures seconds since UNIX epoch.

/// A time-series type for storing a consecutive set of temporal data.
#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct TimeSeries<T> {
    pub timestamps: Vec<T>,
}

/// Implementations for TimeSeries
impl<T: Default> TimeSeries<T> {
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

    /// Get current length of the TimeSeries
    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    pub fn capacity(&self) -> usize {
        self.timestamps.capacity()
    }

    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity()
    }

    /// Check if the TimeSeries is empty
    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    /// Get a reference to the value at the specified index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.timestamps.get(index)
    }

    /// Set the value at an index. Expand vector to capacity if needed.
    pub fn set(&mut self, index: usize, value: T) {
        if index < self.timestamps.capacity() {
            if index < self.timestamps.len() {
                self.timestamps[index] = value;
            } else {
                // Fill in any gaps with default values
                while self.timestamps.len() < index {
                    // Assuming T: Default
                    self.timestamps.push(Default::default());
                }
                self.timestamps.push(value);
            }
        }
    }

    /// Push a new value to the end of the TimeSeries
    pub fn push(&mut self, value: T) {
        self.timestamps.push(value);
    }

    /// Get an iterator over the TimeSeries
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.timestamps.iter()
    }

    /// Slice the TimeSeries over a given range
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self
    where
        T: Clone,
    {
        TimeSeries {
            timestamps: self.timestamps[range].to_vec(),
        }
    }
}

#[test]
fn test_time_series() {
    let mut ts = TimeSeries::from_len(5);

    // Capacity of vectors can mean length is 0
    assert_eq!(ts.len(), 0);
    assert!(ts.is_empty());

    ts.set(0, 10);
    ts.set(1, 20);
    ts.push(30);

    assert_eq!(ts.get(0), Some(&10));
    assert_eq!(ts.get(1), Some(&20));
    assert_eq!(ts.get(5), None);

    let sliced_ts = ts.slice(0..2);
    assert_eq!(sliced_ts.len(), 2);
    assert_eq!(sliced_ts.get(0), Some(&10));
    assert_eq!(sliced_ts.get(1), Some(&20));
}

#[test]
fn test_slicing() {
    let mut ts = TimeSeries::from_len(10);
    for i in 0..10 {
        ts.push(i);
    }

    let sliced_ts = ts.slice(2..5);
    assert_eq!(sliced_ts.len(), 3);
    assert_eq!(sliced_ts.get(0), Some(&2));
    assert_eq!(sliced_ts.get(1), Some(&3));
    assert_eq!(sliced_ts.get(2), Some(&4));
}

#[test]
fn test_capacity_and_isfull() {
    let mut ts = TimeSeries::from_len(3);
    assert_eq!(ts.capacity(), 3);
    assert!(!ts.is_full());

    ts.push(1);
    ts.push(2);
    ts.push(3);
    assert!(ts.is_full());
}
