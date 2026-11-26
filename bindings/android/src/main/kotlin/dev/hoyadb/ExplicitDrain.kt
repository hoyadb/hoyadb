package dev.hoyadb

// d667c6f: exact same owned prefix/liveness dispatcher as automatic notifications.
suspend fun Database.drainNotifications() {
    current()
    observers().explicitDrain()
}
