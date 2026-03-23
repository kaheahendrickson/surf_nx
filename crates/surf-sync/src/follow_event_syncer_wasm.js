import { createInbox, wsconnect } from "@nats-io/nats-core";
import { AckPolicy, DeliverPolicy, jetstream, jetstreamManager } from "@nats-io/jetstream";

const IDLE_HEARTBEAT_NS = 30_000_000_000;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return { promise, resolve, reject };
}

function enqueueMessage(state, message) {
  const waiter = state.waiters.shift();
  if (waiter) {
    waiter.resolve(message);
    return;
  }

  state.buffer.push(message);
}

function failSubscription(state, error) {
  state.closed = true;
  state.error = error instanceof Error ? error : new Error(String(error));
  while (state.waiters.length > 0) {
    const waiter = state.waiters.shift();
    waiter.reject(state.error);
  }
}

async function deleteConsumerIfPresent(client, streamName, consumerName) {
  try {
    await client.jsm.consumers.delete(streamName, consumerName);
  } catch (_error) {
    // Ignore missing durable consumers so a fresh one can be created.
  }
}

async function closeSubscription(state) {
  state.closed = true;
  if (state.messages) {
    await state.messages.close();
  }
}

async function ensurePushConsumer(client, streamName, consumerName, subjects, startSequence) {
  await deleteConsumerIfPresent(client, streamName, consumerName);

  const config = {
    durable_name: consumerName,
    ack_policy: AckPolicy.Explicit,
    deliver_policy: typeof startSequence === "number" ? DeliverPolicy.StartSequence : DeliverPolicy.All,
    deliver_subject: createInbox(),
    replay_policy: "instant",
    flow_control: true,
    idle_heartbeat: IDLE_HEARTBEAT_NS,
  };

  if (subjects.length === 1) {
    config.filter_subject = subjects[0];
  } else {
    config.filter_subjects = subjects;
  }

  if (typeof startSequence === "number") {
    config.opt_start_seq = Number(startSequence);
  }

  await client.jsm.consumers.add(streamName, config);
}

async function pumpSubscription(client, subscriptionId, state) {
  try {
    for await (const message of state.messages) {
      const ackId = `${subscriptionId}:${client.nextAckId++}`;
      client.deliveries.set(ackId, message);
      enqueueMessage(state, {
        ackId,
        payload: message.data,
        streamSequence: message.seq,
      });
    }

    failSubscription(state, new Error(`event stream subscription closed for ${subscriptionId}`));
  } catch (error) {
    failSubscription(state, error);
  }
}

export async function connectEventStreamClient(url) {
  const nc = await wsconnect({
    servers: url,
    ignoreServerUpdates: true,
  });

  return {
    nc,
    js: jetstream(nc),
    jsm: await jetstreamManager(nc),
    deliveries: new Map(),
    subscriptions: new Map(),
    nextAckId: 1,
  };
}

export async function subscribeEventConsumer(
  client,
  streamName,
  consumerName,
  subjects,
  startSequence,
) {
  const normalizedSubjects = Array.from(subjects ?? [], (value) => String(value)).filter(Boolean);
  if (normalizedSubjects.length === 0) {
    throw new Error(`missing event subjects for ${consumerName}`);
  }

  const existing = client.subscriptions.get(consumerName);
  if (existing) {
    await closeSubscription(existing);
    client.subscriptions.delete(consumerName);
  }

  await ensurePushConsumer(client, streamName, consumerName, normalizedSubjects, startSequence ?? undefined);

  const consumer = await client.js.consumers.getPushConsumer(streamName, consumerName);
  const messages = await consumer.consume();
  const state = {
    buffer: [],
    waiters: [],
    closed: false,
    error: null,
    messages,
  };
  client.subscriptions.set(consumerName, state);
  void pumpSubscription(client, consumerName, state);
  return consumerName;
}

export async function nextEventMessage(client, subscriptionId) {
  const state = client.subscriptions.get(subscriptionId);
  if (!state) {
    throw new Error(`missing event subscription ${subscriptionId}`);
  }

  if (state.buffer.length > 0) {
    return state.buffer.shift();
  }

  if (state.error) {
    throw state.error;
  }

  const waiter = deferred();
  state.waiters.push(waiter);
  return await waiter.promise;
}

export async function ackEventMessage(client, ackId) {
  const message = client.deliveries.get(ackId);
  if (!message) {
    throw new Error(`missing event delivery for ack id ${ackId}`);
  }

  client.deliveries.delete(ackId);
  message.ack();
}

export function emitSyncUpdate(domain) {
  if (typeof globalThis.postMessage === "function") {
    globalThis.postMessage({ type: "sync-update", domain });
  }
}
