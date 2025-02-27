use {
    crate::SparseMatrix,
    acir::{
        circuit::{Circuit, Opcode},
        native_types::{Expression, Witness},
        AcirField, FieldElement,
    },
    std::collections::BTreeMap,
};

/// Represents a R1CS constraint system.
#[derive(Debug, Clone)]
pub struct R1CS {
    pub a: SparseMatrix<FieldElement>,
    pub b: SparseMatrix<FieldElement>,
    pub c: SparseMatrix<FieldElement>,

    // Remapping of witness indices to the r1cs_witness array
    pub witnesses: usize,
    pub remap:     BTreeMap<usize, usize>,

    pub constraints: usize,
}

impl R1CS {
    pub fn new() -> Self {
        Self {
            a:           SparseMatrix::new(0, 1, FieldElement::zero()),
            b:           SparseMatrix::new(0, 1, FieldElement::zero()),
            c:           SparseMatrix::new(0, 1, FieldElement::zero()),
            witnesses:   1,
            remap:       BTreeMap::new(),
            constraints: 0,
        }
    }

    pub fn add_circuit(&mut self, circuit: &Circuit<FieldElement>) {
        for opcode in circuit.opcodes.iter() {
            match opcode {
                Opcode::AssertZero(expr) => self.add_assert_zero(expr),

                // TODO: Brillig is a VM used to generate witness values. It does not produce
                // constraints.
                Opcode::BrilligCall { .. } => unimplemented!("BrilligCall"),

                // Directive is a modern version of Brillig.
                Opcode::Directive(..) => unimplemented!("Directive"),

                // Calls to a function, this is to efficiently represent repeated structure in
                // circuits. TODO: We need to implement this so we can store
                // circuits concicely. It should not impact the R1CS constraints or
                // witness vector.
                Opcode::Call { .. } => unimplemented!("Call"),

                // These should be implemented using lookup arguments, or memory checking arguments.
                Opcode::MemoryOp { .. } => unimplemented!("MemoryOp"),
                Opcode::MemoryInit { .. } => unimplemented!("MemoryInit"),

                // These are calls to built-in functions, for this we need to create.
                Opcode::BlackBoxFuncCall(_) => unimplemented!("BlackBoxFuncCall"),
            }
        }
    }

    /// Index of the constant one witness
    pub fn witness_one(&self) -> usize {
        0
    }

    /// Create a new witness variable
    pub fn new_witness(&mut self) -> usize {
        let value = self.witnesses;
        self.witnesses += 1;
        self.a.grow(self.constraints, self.witnesses);
        self.b.grow(self.constraints, self.witnesses);
        self.c.grow(self.constraints, self.witnesses);
        value
    }

    /// Map ACIR Witnesses to r1cs_witness indices
    pub fn map_witness(&mut self, witness: Witness) -> usize {
        self.remap
            .get(&witness.as_usize())
            .copied()
            .unwrap_or_else(|| {
                let value = self.new_witness();
                self.remap.insert(witness.as_usize(), value);
                value
            })
    }

    /// Add an R1CS constraint.
    pub fn add_constraint(
        &mut self,
        a: &[(FieldElement, usize)],
        b: &[(FieldElement, usize)],
        c: &[(FieldElement, usize)],
    ) {
        let row = self.constraints;
        self.constraints += 1;
        self.a.grow(self.constraints, self.witnesses);
        self.b.grow(self.constraints, self.witnesses);
        self.c.grow(self.constraints, self.witnesses);
        for (c, col) in a.iter().copied() {
            self.a.set(row, col, c)
        }
        for (c, col) in b.iter().copied() {
            self.b.set(row, col, c)
        }
        for (c, col) in c.iter().copied() {
            self.c.set(row, col, c)
        }
    }

    /// Add an ACIR assert zero constraint.
    pub fn add_assert_zero(&mut self, expr: &Expression<FieldElement>) {
        // Create individual constraints for all the multiplication terms and collect
        // their outputs
        let mut linear = expr
            .mul_terms
            .iter()
            .map(|term| {
                let a = self.map_witness(term.1);
                let b = self.map_witness(term.2);
                let c = self.new_witness();
                self.add_constraint(&[(FieldElement::one(), a)], &[(FieldElement::one(), b)], &[
                    (FieldElement::one(), c),
                ]);
                (term.0, c)
            })
            .collect::<Vec<_>>();

        // Extend with linear combinations
        linear.extend(
            expr.linear_combinations
                .iter()
                .map(|term| (term.0, self.map_witness(term.1))),
        );

        // Add constant by multipliying with constant value one.
        linear.push((expr.q_c, self.witness_one()));

        // Add a single linear constraint
        // We could avoid this by substituting back into the last multiplication
        // constraint.
        self.add_constraint(&[], &[], &linear);
    }
}
