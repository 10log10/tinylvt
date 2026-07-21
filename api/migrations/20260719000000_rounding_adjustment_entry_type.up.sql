-- Balanced per-community rounding adjustment entries, used to quantize
-- account balances to the community's currency_minor_units (one-time dust
-- migration at rollout, and re-run when minor_units is coarsened).
ALTER TYPE entry_type ADD VALUE 'rounding_adjustment';

-- Dead column: never written, permanently at its default. Balances live on
-- per-community accounts (accounts.balance_cached).
ALTER TABLE users DROP COLUMN balance;
