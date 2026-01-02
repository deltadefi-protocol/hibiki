use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment variables globally.
/// This function is safe to call multiple times - it only runs once.
pub fn init_test_env() {
    INIT.call_once(|| {
        dotenv::dotenv().ok();

        unsafe {
            std::env::set_var(
                "DEX_ORACLE_NFT",
                "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d",
            );
            std::env::set_var(
                "USDM_UNIT",
                "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            );
            std::env::set_var(
                "NIGHT_UNIT",
                "3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854",
            );
            std::env::set_var(
                "IAG_UNIT",
                "82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147",
            );
            std::env::set_var(
                "SNEK_UNIT",
                "378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b",
            );
            std::env::set_var(
                "HOSKY_UNIT",
                "a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59",
            );
            std::env::set_var(
                "OWNER_VKEY",
                "fa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c",
            );
            std::env::set_var(
                "APP_OWNER_SEED_PHRASE",
                "trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade",
            );
            std::env::set_var(
                "FEE_COLLECTOR_SEED_PHRASE",
                "summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer",
            );
        }
    });
}
