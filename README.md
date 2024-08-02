# transaction-processing

[![Build Status](https://github.com/sabin-rapan/transaction-processing/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sabin-rapan/transaction-processing/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/sabin-rapan/transaction-processing/branch/main/graph/badge.svg)](https://codecov.io/gh/sabin-rapan/transaction-processing)

## Overview

Binary application that processes transactions (in the form of CSV records) and
outputs the balance of accounts.   

### Architecture

In a production environment, this would've likely been a TCP server or even
better implemented via managed Cloud Services.   
While the current implemention is far from anything production worthy, it tries
to be somewhat compatible at least in spirit.   
It tries to split the code into client/server, client reads CSV records and
sends them to the server.   
The server receives each record in the order they were sent (as we don't want
to screw up a client's account by doing something else than they asked).   
Then it dispatches each record to a per account handler which processes it and
updates the storage (sharded hashmap with account id as partition key).   
In the "real world", the storage would've been a managed Cloud DB Service which
bares similarities to a sharded hashmap (i.e. one can lock a table row via a
unique partition key, allowing multiple records to be processed concurrently).   
This server <-> N handlers (implemented here via mpsc::channel and oneshot::channel)
model tries to be akin to a queue which fans out requests to workers, though
its implementation has wrinkles.

### Resource Utilization

#### Memory

Each account holds the history of transactions (deposits, withdrawals) for the
purpose of handling disputes/charge backs.   
Currently, this history is unbounded which means the application could
potentially run out of memory on a machine. In a production environment, this
would be handled by ageing transactions which go over a given charge back
period (60 days in most US and EU legislations).

#### CPU

The server spawns a tokio::task for each account, which means the application
could potentially create too many tasks compared to the CPU cores/threads the
system has, leading to a poor customer experience.   
The number of tasks should be rate limited so that the server can guarantee
timely transaction processing and good customer experience.

## Dependencies

* `clap` for input parameters processing (while a heavy crate, usually CLI
  applications are frequently extended and clap facilitates this)
* `tokio` and friends for task runtime
* `csv-async` for handling CSV records
* `serse` for serializing/deserialing structs into CSV records and vice-verse
* `rust_decimal` for representing account balance (a bit overkill, but avoids
  the non-sense that is f64 and faster to use compared to implementing from
  scratch a la bitcoin crate)
* `dashmap` concurrent hashmap shared across mulwtiple tasks (in reality all
  that was needed was sharding via a series of RwLocks, but using dashmap is
  faster for prototyping)
* `thiserror` because it reduces boiler plate from implementing Display for
  each module/crate Error (and the crate is tiny).
* `tracing` for nice stdout logs during debugging

Overall, a bit heavier in dependencies than I would've liked, but it's a small
price to pay in order to type faster.   
In a production environment, serious care should be exercised when consuming
3rd party crates.

## TODOs 
* Rate limit account handlers (e.g. either via a semaphore as tokio does it in
  its tutorial, or a token bucket algorithm).
* Somewhat tied to the above, close and drop handlers if their channel doesn't
  have messages pending. Optimization of host resources for when the customer
  usage pattern is infrequent.
* Handle CTRL+C. Strictly speaking it's not necessary in this application
  because the storage is forever lost whenever the process exits, but if this
  were a TCP server deployed in a docker container somewhere in a fleet, the
  server could receive SIGKILL from the OOM killer in Linux if it generates too
  much memory pressure and we don't want that to happen in the middle of a
  write to our data store.
* Improve testing coverage (currently sitting around 80%)
* Clean up client/server interfaces. Currently its a mpsc::channel, should be
  an implemetor of Trait AsyncRead and AsyncWrite respectively.
* Maybe improve client code to send records in parallel for each client
  account, instead of sequentially.
