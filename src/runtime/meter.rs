use crate::error::{StrataError, StrataResult};

/// Deterministic instruction budget for WebAssembly guest execution.
#[derive(Debug, Clone, Copy)]
pub struct FuelBudget {
    /// Initial fuel units allocated to the execution context.
    pub initial_fuel: u64,
    /// Absolute ceiling limit to prevent rogue runaway scripts.
    pub max_fuel_limit: u64,
    /// Cost multiplier for expensive operations (e.g., host function calls).
    pub host_call_cost: u64,
}

impl Default for FuelBudget {
    fn default() -> Self {
        Self {
            initial_fuel: 1_000_000,   // 1M fuel units default
            max_fuel_limit: 10_000_000, // 10M fuel ceiling
            host_call_cost: 100,        // 100 fuel units per host boundary transition
        }
    }
}

impl FuelBudget {
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            initial_fuel,
            max_fuel_limit: initial_fuel.saturating_mul(2),
            host_call_cost: 100,
        }
    }

    pub fn with_ceil(mut self, max_limit: u64) -> Self {
        self.max_fuel_limit = max_limit;
        self
    }
}

/// Dynamic fuel and instruction meter for fine-grained budget tracking.
#[derive(Debug, Clone)]
pub struct FuelMeter {
    budget: FuelBudget,
    fuel_remaining: u64,
    fuel_consumed: u64,
}

impl FuelMeter {
    pub fn new(budget: FuelBudget) -> Self {
        let initial = budget.initial_fuel.min(budget.max_fuel_limit);
        Self {
            budget,
            fuel_remaining: initial,
            fuel_consumed: 0,
        }
    }

    pub fn remaining(&self) -> u64 {
        self.fuel_remaining
    }

    pub fn consumed(&self) -> u64 {
        self.fuel_consumed
    }

    /// Charge fuel units for guest execution or host calls.
    /// Returns Err if the budget is exhausted.
    pub fn charge(&mut self, units: u64) -> StrataResult<u64> {
        if self.fuel_remaining < units {
            self.fuel_consumed = self.fuel_consumed.saturating_add(self.fuel_remaining);
            let attempted_total = self.fuel_consumed.saturating_add(units - self.fuel_remaining);
            self.fuel_remaining = 0;
            return Err(StrataError::FuelExhausted {
                consumed: attempted_total,
                limit: self.budget.initial_fuel,
            });
        }

        self.fuel_remaining -= units;
        self.fuel_consumed = self.fuel_consumed.saturating_add(units);
        Ok(self.fuel_remaining)
    }

    /// Charge fuel specifically for a host capability call.
    pub fn charge_host_call(&mut self) -> StrataResult<u64> {
        self.charge(self.budget.host_call_cost)
    }

    /// Replenish fuel up to the maximum allowable limit.
    pub fn replenish(&mut self, units: u64) -> u64 {
        let room = self.budget.max_fuel_limit.saturating_sub(self.fuel_remaining);
        let to_add = units.min(room);
        self.fuel_remaining = self.fuel_remaining.saturating_add(to_add);
        self.fuel_remaining
    }

    /// Check if the meter is depleted.
    pub fn is_depleted(&self) -> bool {
        self.fuel_remaining == 0
    }
}
