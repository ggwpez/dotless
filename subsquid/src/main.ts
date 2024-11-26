import {TypeormDatabase, Store} from '@subsquid/typeorm-store'
import {In} from 'typeorm'
import * as ss58 from '@subsquid/ss58'
import assert from 'assert'

import {processor, ProcessorContext} from './processor'
import { EraPaid } from './model'
import {events} from './types'
import { totalIssuance } from './types/balances/storage'

processor.run(new TypeormDatabase({ supportHotBlocks: true }), async (ctx) => {
    let pays: EraPaid[] = await getPays(ctx)
    await ctx.store.save(pays)
})

// Extract all Staking::EraPaid events.
async function getPays(ctx: ProcessorContext<Store>): Promise<EraPaid[]> {
    let pays: EraPaid[] = []

    for (let block of ctx.blocks) {
        if (block.header.height < 23467056)
            continue
        const ti = totalIssuance.v0.get(block.header)

        for (let event of block.events) {
            if (event.name != events.staking.eraPaid.name)
                continue

            let minted: bigint = 0n

            if (events.staking.eraPaid.v9300.is(event)) {
                const { validatorPayout, remainder } = events.staking.eraPaid.v9300.decode(event)
                minted = validatorPayout + remainder
            } else {
                throw new Error('Unsupported spec')
            }

            assert(block.header.timestamp, `Got an undefined timestamp at block ${block.header.height}`)

            pays.push(new EraPaid({
                id: event.id,
                blockNumber: block.header.height,
                timestamp: new Date(block.header.timestamp),
                amountPaid: minted,
                totalIssuance: await ti,
            }))
        }
    }
    return pays
}
