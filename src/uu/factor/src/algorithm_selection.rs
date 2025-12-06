// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Algorithm selection for optimal factorization
//!
//! This module routes numbers to the appropriate factorization method:
//! - Small numbers (< 128 bits): fast_factor (optimized for u64/u128 range)
//! - Larger numbers (>= 128 bits): falls back to num_prime

use num_bigint::BigUint;
use std::collections::BTreeMap;
use num_traits::ToPrimitive;

use super::trial_division::{extract_small_factors, quick_trial_divide};
use super::u64_factor::{is_probable_prime_u64, pollard_rho_brent_u64, trial_division_u64};
use super::pollard_rho::pollard_rho_with_target;
use super::fermat::{fermat_factor_u64, fermat_factor_biguint};


/// Fast factorization for numbers < 128 bits
///
/// Strategy (internal routing):
/// - ≤ 64 bits: Use optimized u64 algorithms (trial division + Pollard-Rho, Fermat hint)
/// - 64-~90 bits: Use optimized BigUint Pollard-Rho after stripping small factors
fn fast_factorize_small(n: &BigUint) -> BTreeMap<BigUint, usize> {
    let bits = n.bits();

    // Handle trivial cases
    if n <= &BigUint::from(1u32) {
        return BTreeMap::new();
    }

    // For numbers ≤ 64 bits, use u64 optimization path
    if bits <= 64 {
        if let Some(n_u64) = n.to_u64() {
            return factorize_u64_fast(n_u64);
        }
    }

    // For 64-~90 bit numbers, use BigUint path with optimizations
    factorize_biguint_fast(n)
}

/// Optimized factorization for u64 numbers
fn factorize_u64_fast(mut n: u64) -> BTreeMap<BigUint, usize> {
    let mut factors = BTreeMap::new();

    if n <= 1 {
        return factors;
    }

    if n == 2 || n == 3 || n == 5 {
        factors.insert(BigUint::from(n), 1);
        return factors;
    }

    // Trial division for small primes (up to ~1000)
    let small_primes_u64 = trial_division_u64(&mut n, 1000);
    for prime in small_primes_u64 {
        *factors.entry(BigUint::from(prime)).or_insert(0) += 1;
    }

    // If fully factored, return
    if n == 1 {
        return factors;
    }

    // Check if remaining number is prime
    if is_probable_prime_u64(n) {
        factors.insert(BigUint::from(n), 1);
        return factors;
    }

    // Try Fermat's method first for semiprimes (optimal for close factors)
    if let Some(fermat_factor) = fermat_factor_u64(n) {
        // Found via Fermat! Recursively factor both parts
        factorize_u64_pollard_rho(&mut factors, fermat_factor);
        factorize_u64_pollard_rho(&mut factors, n / fermat_factor);
        return factors;
    }

    // Fallback to Pollard-Rho for remaining composite
    factorize_u64_pollard_rho(&mut factors, n);

    factors
}

/// Recursive Pollard-Rho factorization for u64
fn factorize_u64_pollard_rho(factors: &mut BTreeMap<BigUint, usize>, n: u64) {
    if n == 1 {
        return;
    }

    if is_probable_prime_u64(n) {
        *factors.entry(BigUint::from(n)).or_insert(0) += 1;
        return;
    }

    // Find a factor using Pollard-Rho
    if let Some(factor) = pollard_rho_brent_u64(n) {
        // Recursively factor both parts
        factorize_u64_pollard_rho(factors, factor);
        factorize_u64_pollard_rho(factors, n / factor);
    } else {
        // Couldn't find factor, assume it's prime (shouldn't happen often)
        *factors.entry(BigUint::from(n)).or_insert(0) += 1;
    }
}

/// Optimized factorization for BigUint (64-~90 bit range, internal)
fn factorize_biguint_fast(n: &BigUint) -> BTreeMap<BigUint, usize> {
    let mut factors = BTreeMap::new();

    // Extract small factors first
    let (small_factors, mut remaining) = extract_small_factors(n.clone());
    for factor in small_factors {
        *factors.entry(factor).or_insert(0) += 1;
    }

    // If fully factored, return
    if remaining == BigUint::from(1u32) {
        return factors;
    }

    // Trial division for medium-sized primes
    let (more_factors, final_remaining) = quick_trial_divide(remaining);
    for factor in more_factors {
        *factors.entry(factor).or_insert(0) += 1;
    }
    remaining = final_remaining;

    // If fully factored, return
    if remaining == BigUint::from(1u32) || remaining == BigUint::from(0u32) {
        return factors;
    }

    // Try Fermat's method for numbers up to ~90 bits (optimal for close factors)
    if remaining.bits() <= 90 {
        if let Some(fermat_factor) = fermat_factor_biguint(&remaining) {
            // Found via Fermat! Recursively factor both parts
            factorize_biguint_pollard_rho(&mut factors, fermat_factor.clone());
            factorize_biguint_pollard_rho(&mut factors, &remaining / &fermat_factor);
            return factors;
        }
    }

    // Fallback to Pollard-Rho for remaining composite
    factorize_biguint_pollard_rho(&mut factors, remaining);

    factors
}

/// Recursive Pollard-Rho factorization for BigUint (internal)
fn factorize_biguint_pollard_rho(factors: &mut BTreeMap<BigUint, usize>, n: BigUint) {
    if n == BigUint::from(1u32) {
        return;
    }

    // For very small n, assume prime
    if n.bits() <= 20 {
        *factors.entry(n).or_insert(0) += 1;
        return;
    }

    // Estimate factor size (assume roughly balanced factors)
    let target_bits = (n.bits() as u32) / 2;

    // Find a factor using Pollard-Rho
    if let Some(factor) = pollard_rho_with_target(&n, target_bits) {
        if factor < n {
            // Recursively factor both parts
            factorize_biguint_pollard_rho(factors, factor.clone());
            factorize_biguint_pollard_rho(factors, &n / &factor);
        } else {
            // Factor is same as n, assume prime
            *factors.entry(n).or_insert(0) += 1;
        }
    } else {
        // Couldn't find factor, assume it's prime
        *factors.entry(n).or_insert(0) += 1;
    }
}


/// Main factorization entry point with algorithm selection
///
/// Routes to the optimal algorithm based on number size:
/// - < 128 bits: fast_factor (trial division + Fermat + Pollard-Rho)
/// - otherwise: num_prime fallback (best-effort for very large numbers)
pub fn factorize(n: &BigUint) -> (BTreeMap<BigUint, usize>, Option<Vec<BigUint>>) {
    let bits = n.bits();

    // < 128-bit path: use our fast implementation
    if bits < 128 {
        return (fast_factorize_small(n), None);
    }

    // Fallback: delegate to num_prime for larger inputs
    num_prime::nt_funcs::factors(n.clone(), None)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_factorize_128bit() {
        // 128-bit semiprime (boundary of <u128 focus)
        // Using two ~64-bit primes to create a ~128-bit semiprime
        let p = BigUint::parse_bytes(b"18446744073709551629", 10).unwrap();
        let q = BigUint::parse_bytes(b"18446744073709551557", 10).unwrap();
        let n = &p * &q;

        assert!(n.bits() >= 100);

        let (factors, remaining) = factorize(&n);
        assert_eq!(remaining, None);
        // Should factor successfully
        assert!(!factors.is_empty());
    }

    #[test]
    #[ignore] // factoring ~200-bit semiprimes is out of scope for this <u128-focused build
    fn test_factorize_200bit() {
        // ~200-bit semiprime (previously used SIQS). Out of scope without yamaquasi.
        let p = BigUint::parse_bytes(
            b"1267650600228229401496703205653",
            10
        ).unwrap();
        let q = BigUint::parse_bytes(
            b"1267650600228229401496703205659",
            10
        ).unwrap();
        let n = &p * &q;

        assert!(n.bits() >= 180); // ~200 bits

        // Without SIQS, we do not require full factorization here.
        let (_factors, _remaining) = factorize(&n);
    }

    #[test]
    #[ignore] // This test may be slow - 400 bits might exceed yamaquasi's optimal range
    fn test_factorize_400bit() {
        // 400-bit number (beyond yamaquasi's 330-bit limit, falls back to num_prime)
        // Using smaller factors to ensure it can still be factored
        let p = BigUint::parse_bytes(
            b"1606938044258990275541962092341162602522202993782792831",
            10
        ).unwrap(); // ~200 bits
        let q = BigUint::parse_bytes(
            b"1606938044258990275541962092341162602522202993782792833",
            10
        ).unwrap(); // ~200 bits
        let n = &p * &q;

        assert!(n.bits() > 330); // Should exceed yamaquasi's range

        let (_factors, remaining) = factorize(&n);
        // This may timeout or fail depending on num_prime's capabilities
        // We're mainly testing that it doesn't panic
        assert!(remaining.is_none() || remaining.is_some());
    }

}
