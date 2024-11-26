module.exports = class Data1732469689534 {
    name = 'Data1732469689534'

    async up(db) {
        await db.query(`CREATE TABLE "era_paid" ("id" character varying NOT NULL, "block_number" integer NOT NULL, "timestamp" TIMESTAMP WITH TIME ZONE NOT NULL, "amount_paid" numeric NOT NULL, "total_issuance" numeric NOT NULL, CONSTRAINT "PK_473fd2a58b7581ecb1702bb2d80" PRIMARY KEY ("id"))`)
        await db.query(`CREATE INDEX "IDX_47f91641dc82decde068064b95" ON "era_paid" ("block_number") `)
        await db.query(`CREATE INDEX "IDX_501a671b2c3161420d9e1fdf49" ON "era_paid" ("timestamp") `)
    }

    async down(db) {
        await db.query(`DROP TABLE "era_paid"`)
        await db.query(`DROP INDEX "public"."IDX_47f91641dc82decde068064b95"`)
        await db.query(`DROP INDEX "public"."IDX_501a671b2c3161420d9e1fdf49"`)
    }
}
