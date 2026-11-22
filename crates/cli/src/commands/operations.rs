//! Operational commands validate exact argument shapes and finite budgets before any source access.
use std::path::Path;

/// Keep retention maintenance separate from query filters and ledger command submission.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    match (action, args) {
        ("check-config"|"preflight",[path,profile_flag,profile]) if profile_flag=="--profile"=>{
            let profile=cantongrid_runtime::NetworkProfile::parse(profile)?;
            if action=="check-config"{return crate::print_json(&cantongrid_runtime::check_configuration(Path::new(path),profile)?);}
            let runtime=tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|_|"runtime unavailable")?;
            crate::print_json(&runtime.block_on(cantongrid_runtime::preflight(Path::new(path),profile))?)
        }

        ("prune-deliveries", [path, retain_flag, retain, limit_flag, limit])
            if retain_flag == "--retain" && limit_flag == "--limit" =>
        {
            let retain = retain
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=100000).contains(n))
                .ok_or("invalid delivery retention budget")?;
            let limit = limit
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=256).contains(n))
                .ok_or("invalid delivery retention budget")?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| "runtime unavailable")?;
            crate::print_json(&runtime.block_on(cantongrid_runtime::prune_deliveries(
                Path::new(path),
                retain,
                limit,
            ))?)
        }
        _ => Err(
            "expected ops check-config|preflight <runtime> --profile localnet|testnet|mainnet or ops prune-deliveries <runtime> --retain <1..100000> --limit <1..256>".into(),
        ),
    }
}
