use scrypto::prelude::*;

#[blueprint]
mod radiswap_module {
    struct Radiswap {
        vault_a: Vault,
        vault_b: Vault,
        pool_units_resource_address: ResourceAddress,
        pool_units_minter_badge: Vault,
        fee: Decimal,
    }

    impl Radiswap {
        pub fn instantiate_radiswap(
            bucket_a: Bucket,
            bucket_b: Bucket,
            fee: Decimal,
        ) -> (ComponentAddress, Bucket) {
            assert!(
                !bucket_a.is_empty() && !bucket_b.is_empty(),
                "You must pass an initial supply of tokens to the pool"
            );
            assert!(
                fee >= dec!("0") && fee <= dec!("1"),
                "Fee must be between 0 and 1"
            );
            let pool_units_minter_badge: Bucket = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "LP token mint Auth")
                .mint_initial_supply(1);

            let pool_units: Bucket = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata("name", "Pool unit")
                .metadata("symbol", "UNIT")
                .mintable(
                    rule!(require(pool_units_minter_badge.resource_address())),
                    LOCKED,
                )
                .burnable(
                    rule!(require(pool_units_minter_badge.resource_address())),
                    LOCKED,
                )
                .mint_initial_supply(100);

            let radiswap: ComponentAddress = Self {
                vault_a: Vault::with_bucket(bucket_a),
                vault_b: Vault::with_bucket(bucket_b),
                fee,
                pool_units_resource_address: pool_units.resource_address(),
                pool_units_minter_badge: Vault::with_bucket(pool_units_minter_badge),
            }
            .instantiate()
            .globalize();
            (radiswap, pool_units)
        }
        pub fn swap(&mut self, input_tokens: Bucket) -> Bucket {
            // checking if the input token is one of the two tokens in the pool
            let (input_tokens_vault, output_token_vault): (&mut Vault, &mut Vault) =
                if input_tokens.resource_address() == self.vault_a.resource_address() {
                    (&mut self.vault_a, &mut self.vault_b)
                } else if input_tokens.resource_address() == self.vault_b.resource_address() {
                    (&mut self.vault_b, &mut self.vault_a)
                } else {
                    panic!("Invalid input token")
                };
            // Applying the Constant Product Market Maker formula
            let output_amount: Decimal = (output_token_vault.amount()
                * (dec!("1") - self.fee)
                * input_tokens.amount())
                / (input_tokens_vault.amount() + input_tokens.amount() * (dec!("1") - self.fee));
            // Transfering the input tokens to the pool
            input_tokens_vault.put(input_tokens);

            (output_token_vault.take(output_amount))
        }
        pub fn add_liquidity(
            &mut self,
            bucket_a: Bucket,
            bucket_b: Bucket,
        ) -> (Bucket, Bucket, Bucket) {
            let (mut bucket_a, mut bucket_b): (Bucket, Bucket) =
                if bucket_a.resource_address() == self.vault_a.resource_address() {
                    (bucket_a, bucket_b)
                } else if bucket_a.resource_address() == self.vault_b.resource_address() {
                    (bucket_b, bucket_a)
                } else {
                    panic!("Invalid input token, one of the tokens does not belong to the pool")
                };
            let dm: Decimal = bucket_a.amount();
            let dn: Decimal = bucket_b.amount();
            let m: Decimal = self.vault_a.amount();
            let n: Decimal = self.vault_b.amount();

            let (amount_a, amount_b): (Decimal, Decimal) =
                if ((m == Decimal::zero()) || (n == Decimal::zero()) || ((m / n) == (dm / dn))) {
                    (dm, dn)
                } else if (m / n) < (dm / dn) {
                    (dn * m / n, dn)
                } else {
                    (dm, dm * n / m)
                };
            self.vault_a.put(bucket_a.take(amount_a));
            self.vault_b.put(bucket_b.take(amount_b));

            let pool_units_manager: &mut ResourceManager =
                borrow_resource_manager!(self.pool_units_resource_address);
            let pool_units_amount: Decimal = if pool_units_manager.total_supply() == Decimal::zero()
            {
                dec!("100.00")
            } else {
                amount_a * pool_units_manager.total_supply() / m
            };
            let pool_units: Bucket = self
                .pool_units_minter_badge
                .authorize(|| pool_units_manager.mint(pool_units_amount));
            (bucket_a, bucket_b, pool_units)
        }
        /// Removes the amount of funds from the pool corresponding to the pool units.
        pub fn remove_liquidity(&mut self, pool_units: Bucket) -> (Bucket, Bucket) {
            assert!(
                pool_units.resource_address() == self.pool_units_resource_address,
                "Wrong token type passed in"
            );
            let pool_units_resource_manager: &ResourceManager =
                borrow_resource_manager!(self.pool_units_resource_address);
            let share = pool_units.amount() / pool_units_resource_manager.total_supply();
            self.pool_units_minter_badge.authorize(|| pool_units.burn());
            (
                self.vault_a.take(self.vault_a.amount() * share),
                self.vault_b.take(self.vault_b.amount() * share),
            )
        }
    }
}
