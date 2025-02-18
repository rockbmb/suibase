// MTSafeUUID provides:
//   - a get() function that returns both a method_uuid (UUID v4) and a data_uuid (UUID v7).
//   - The method_uuid is initialized once on process startup and changes whenever a data_uuid is
//     unexpectedly sorted lower than the previous one generated (e.g. system time went backward).
//   - Multi-thread protection (Mutex protected).
//
// SingleThreadUUID is same, except the user is responsible for Mutex access.
//
use uuid::Uuid;

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct UuidST {
    method_uuid: Uuid,
    data_uuid: Uuid,
}

impl UuidST {
    pub fn new() -> Self {
        Self {
            method_uuid: Uuid::new_v4(),
            data_uuid: uuid7::new_v7(),
        }
    }

    pub fn get(&self) -> (Uuid, Uuid) {
        (self.method_uuid, self.data_uuid)
    }

    pub fn set(&mut self, other: &Self) {
        self.method_uuid = other.method_uuid;
        self.data_uuid = other.data_uuid;
    }

    pub fn increment(&mut self) {
        let new_data_uuid: Uuid = uuid7::new_v7();

        if new_data_uuid <= self.data_uuid {
            self.method_uuid = Uuid::new_v4();
        }
        self.data_uuid = new_data_uuid;
    }

    pub fn get_method_uuid(&self) -> String {
        Self::short_uuid_string(&self.method_uuid)
    }

    pub fn get_data_uuid(&self) -> String {
        Self::short_uuid_string(&self.data_uuid)
    }

    fn short_uuid_string(uuid: &Uuid) -> String {
        // Make the uuid shorter with Base32 Hex encoding (RFC4648).
        // This UUID remains lexicographically sortable.
        data_encoding::BASE32HEX_NOPAD.encode(uuid.as_bytes())
    }
}

impl Default for UuidST {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_one_thread_uuid() {
        let single_thread_uuid = UuidST::new();
        let mt_safe_uuid = Arc::new(tokio::sync::Mutex::new(single_thread_uuid));
        let mut locked_uuid = mt_safe_uuid.lock().await;
        let mut prev_data_uuid = locked_uuid.data_uuid;
        let initial_method_uuid = locked_uuid.method_uuid;
        let mut prev_uuid_obj = locked_uuid.clone();
        for _ in 0..100000 {
            locked_uuid.increment();
            let (method_uuid, data_uuid) = locked_uuid.get();

            assert_eq!(method_uuid, initial_method_uuid);
            assert!(data_uuid > prev_data_uuid);
            // Verify lexicographic sorting of strings.
            assert!(locked_uuid.get_data_uuid() > prev_uuid_obj.get_data_uuid());

            prev_data_uuid = data_uuid;
            prev_uuid_obj = locked_uuid.clone();
        }
    }

    #[tokio::test]
    async fn test_two_threads_uuid() {
        let single_thread_uuid = UuidST::new();
        let mt_safe_uuid = Arc::new(tokio::sync::Mutex::new(single_thread_uuid));
        let mt_safe_uuid_clone = mt_safe_uuid.clone();

        let (initial_method_uuid, mut prev_data_uuid, mut prev_uuid_obj) = {
            let locked_uuid = mt_safe_uuid.lock().await;
            (
                locked_uuid.method_uuid,
                locked_uuid.data_uuid,
                locked_uuid.clone(),
            )
        };

        let mut prev_uuid_obj_clone = prev_uuid_obj.clone();

        let (_result1, _result2) = tokio::join!(
            async move {
                for _ in 0..100000 {
                    let mut locked_uuid = mt_safe_uuid.lock().await;
                    locked_uuid.increment();
                    let (method_uuid, data_uuid) = locked_uuid.get();

                    assert_eq!(method_uuid, initial_method_uuid);
                    assert!(data_uuid > prev_data_uuid);
                    assert!(locked_uuid.get_data_uuid() > prev_uuid_obj.get_data_uuid());

                    prev_data_uuid = data_uuid;
                    prev_uuid_obj = locked_uuid.clone();
                }
            },
            async move {
                for _ in 0..100000 {
                    let mut locked_uuid = mt_safe_uuid_clone.lock().await;
                    locked_uuid.increment();
                    let (method_uuid, data_uuid) = locked_uuid.get();

                    assert_eq!(method_uuid, initial_method_uuid);
                    assert!(data_uuid > prev_data_uuid);
                    assert!(locked_uuid.get_data_uuid() > prev_uuid_obj_clone.get_data_uuid());

                    prev_data_uuid = data_uuid;
                    prev_uuid_obj_clone = locked_uuid.clone();
                }
            }
        );
    }

    #[tokio::test]
    async fn test_ordering() {
        let mut a = UuidST::new();
        for _ in 0..100000 {
            let prev_a = a.clone();
            a.increment();

            // Test cloning.
            let same_a = a.clone();
            assert_eq!(a, same_a);
            assert_eq!(same_a, a);
            assert!(a <= same_a);
            assert!(a >= same_a);
            assert_eq!(same_a, same_a);
            assert_eq!(same_a.get_data_uuid(), a.get_data_uuid());
            assert_eq!(same_a.get_method_uuid(), a.get_method_uuid());

            // Repeat same cloning tests with individual components with get()
            let (a_method_uuid, a_data_uuid) = a.get();
            let (same_a_method_uuid, same_a_data_uuid) = same_a.get();
            assert_eq!(a_method_uuid, same_a_method_uuid);
            assert_eq!(a_data_uuid, same_a_data_uuid);
            assert!(a_method_uuid <= same_a_method_uuid);
            assert!(a_data_uuid <= same_a_data_uuid);
            assert!(a_method_uuid >= same_a_method_uuid);
            assert!(a_data_uuid >= same_a_data_uuid);

            // Test various operators
            assert_eq!(prev_a, prev_a);
            assert_ne!(prev_a, a);
            assert!(prev_a != a);
            assert!(prev_a < a);
            assert!(a > prev_a);

            // Repeat tests with the cloned one.
            assert_ne!(prev_a, same_a);
            assert!(prev_a != same_a);
            assert!(prev_a < same_a);
            assert!(same_a > prev_a);
        }
    }
}
