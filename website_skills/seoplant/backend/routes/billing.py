"""
SEOplant Backend — Stripe billing service.
"""
import stripe
from fastapi import APIRouter, Depends, HTTPException, Request
from sqlalchemy.orm import Session

from ..auth import get_current_user
from ..database import get_db
from ..models import User, CreditTransaction
from ..config import STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET, STRIPE_PRICE_PRO, STRIPE_PRICE_AGENCY

stripe.api_key = STRIPE_SECRET_KEY

router = APIRouter(prefix="/api/billing", tags=["billing"])

CREDIT_PACKAGES = {
    "500": 10,     # $10 for 500 credits
    "1200": 20,    # $20 for 1,200 credits
    "3000": 50,    # $50 for 3,000 credits
    "10000": 150,  # $150 for 10,000 credits
}


@router.get("/plans")
def get_plans():
    """Return available plans and credit packages."""
    return {
        "plans": [
            {"id": "free", "name": "Free", "price": 0, "credits": 100, "sites": 1},
            {"id": "pro", "name": "Cloud", "price": 79, "credits": 2000, "sites": 10},
            {"id": "agency", "name": "Agency", "price": 199, "credits": 5000, "sites": 50},
        ],
        "credit_packages": [
            {"id": k, "credits": int(k), "price": v}
            for k, v in CREDIT_PACKAGES.items()
        ],
    }


@router.post("/create-checkout")
def create_checkout(
    request: dict,
    user: User = Depends(get_current_user),
):
    """Create a Stripe Checkout session for subscription or credit purchase."""
    price_id = request.get("price_id", "")
    mode = request.get("mode", "subscription")  # "subscription" or "payment"

    try:
        if mode == "subscription":
            session = stripe.checkout.Session.create(
                customer_email=user.email,
                client_reference_id=user.id,
                mode="subscription",
                line_items=[{"price": price_id, "quantity": 1}],
                success_url="https://seoplant.io/dashboard?checkout=success",
                cancel_url="https://seoplant.io/dashboard?checkout=cancelled",
            )
        else:
            # One-time credit purchase
            session = stripe.checkout.Session.create(
                customer_email=user.email,
                client_reference_id=user.id,
                mode="payment",
                line_items=[{"price": price_id, "quantity": 1}],
                success_url="https://seoplant.io/dashboard?checkout=success",
                cancel_url="https://seoplant.io/dashboard?checkout=cancelled",
                metadata={"user_id": user.id, "type": "credit_purchase"},
            )

        return {"url": session.url}

    except stripe.error.StripeError as e:
        raise HTTPException(400, str(e))


@router.post("/webhook")
async def stripe_webhook(request: Request, db: Session = Depends(get_db)):
    """Handle Stripe webhook events."""
    payload = await request.body()
    sig = request.headers.get("stripe-signature", "")

    try:
        event = stripe.Webhook.construct_event(payload, sig, STRIPE_WEBHOOK_SECRET)
    except (ValueError, stripe.error.SignatureVerificationError):
        raise HTTPException(400, "Invalid webhook signature")

    # Handle checkout.session.completed
    if event["type"] == "checkout.session.completed":
        session = event["data"]["object"]
        user_id = session.get("client_reference_id") or session.get("metadata", {}).get("user_id", "")

        if session.get("mode") == "subscription":
            _handle_subscription_created(user_id, session, db)
        else:
            _handle_credit_purchase(user_id, session, db)

    # Handle subscription deleted
    elif event["type"] == "customer.subscription.deleted":
        sub = event["data"]["object"]
        user = db.query(User).filter(User.stripe_subscription_id == sub["id"]).first()
        if user:
            user.plan = "free"
            user.stripe_subscription_id = ""
            db.commit()

    return {"status": "ok"}


def _handle_subscription_created(user_id: str, session: dict, db: Session):
    """Activate subscription for a user."""
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        return

    price_id = session.get("metadata", {}).get("price_id", "")
    if "pro" in price_id.lower() or STRIPE_PRICE_PRO in price_id:
        user.plan = "pro"
        user.credits_remaining += 2000
    elif "agency" in price_id.lower() or STRIPE_PRICE_AGENCY in price_id:
        user.plan = "agency"
        user.credits_remaining += 5000

    user.stripe_customer_id = session.get("customer", "")
    user.stripe_subscription_id = session.get("subscription", "")
    db.commit()


def _handle_credit_purchase(user_id: str, session: dict, db: Session):
    """Add credits from a one-time purchase."""
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        return

    # Credits info is passed via metadata
    credits = int(session.get("metadata", {}).get("credits", "500"))
    user.credits_remaining += credits
    db.add(CreditTransaction(
        user_id=user_id,
        amount=credits,
        operation=f"purchase_{credits}",
        project_id="",
    ))
    db.commit()
